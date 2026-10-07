//! The turn-side hooks: pre-turn, post-turn and compaction.
//!
//! The agent loop calls these; each is bounded, logs instead of failing, and
//! returns nothing the turn depends on. See `crates/openhuman-core/src/agent/
//! session_host/runtime_session.rs` (pre-turn), `memory::bus` (post-turn,
//! through `ConversationTurnCommitted`) and `agent/tinyagents/
//! memory_summarizer.rs` (compaction).
//!
//! Turn indices follow the committed transcript: the user message of the
//! turn that follows `n` committed turns is `2n`, its reply `2n + 1`. They
//! are stable across retries and restarts, which is what lets TinyMemory
//! recognise a retried turn as a duplicate.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tinymemory_api::{ToolCallRef, Turn};
use tinymemory_tools::{Compaction, ContextPack, PostTurn, PreTurn, SessionStart};

use crate::config::Config;
use crate::memory::engine;
use crate::memory::scope::ResolvedIdentity;

use super::{agent_memory_on, jobs, log_only};

/// Opening tag of an injected pack.
pub const OPEN_TAG: &str = "<memory-context>";
/// Closing tag of an injected pack.
pub const CLOSE_TAG: &str = "</memory-context>";

/// Longest tool result line appended to a logged reply, in characters.
const MAX_TOOL_LINE_CHARS: usize = 240;

/// The user message's index for a turn that follows `committed_turns`.
#[must_use]
pub fn user_turn_index(committed_turns: usize) -> u32 {
    u32::try_from(committed_turns.saturating_mul(2)).unwrap_or(u32::MAX - 1)
}

/// The context a turn is given: one rendered pack and what it cites.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TurnPack {
    /// The block, rendered by TinyMemory; never empty.
    pub markdown: String,
    /// Its estimated tokens.
    pub tokens: usize,
    /// The ids of every item it cites, in order.
    pub refs: Vec<String>,
    /// The engine that answered.
    pub engine: String,
    /// The cited items, for the chat's memory chips.
    #[serde(skip)]
    pub citations: Vec<crate::memory::types::TurnCitation>,
}

impl TurnPack {
    fn from_packs(packs: impl IntoIterator<Item = ContextPack>) -> Option<Self> {
        let mut pack: Option<Self> = None;
        for next in packs.into_iter().filter(|pack| !pack.is_empty()) {
            let cited: Vec<crate::memory::types::TurnCitation> = next
                .sections
                .iter()
                .flat_map(|section| section.hits.iter())
                .filter(|hit| next.refs.contains(&hit.id))
                .map(crate::memory::types::TurnCitation::from)
                .collect();
            let refs = next.refs.iter().map(ToString::to_string);
            match &mut pack {
                Some(pack) => {
                    pack.markdown.push_str("\n\n");
                    pack.markdown.push_str(&next.markdown);
                    pack.tokens += next.tokens;
                    for id in refs {
                        if !pack.refs.contains(&id) {
                            pack.refs.push(id);
                        }
                    }
                    for citation in cited {
                        if !pack.citations.iter().any(|known| known.id == citation.id) {
                            pack.citations.push(citation);
                        }
                    }
                }
                None => {
                    pack = Some(Self {
                        markdown: next.markdown,
                        tokens: next.tokens,
                        refs: refs.collect(),
                        engine: next.engine,
                        citations: cited,
                    });
                }
            }
        }
        pack
    }

    /// The block as the model sees it.
    #[must_use]
    pub fn injection(&self) -> String {
        format!("{OPEN_TAG}\n{}\n{CLOSE_TAG}", self.markdown.trim())
    }
}

/// One turn's memory, carried on its run context: what the compaction hook
/// needs to recall for this thread under this identity, and the pack the
/// turn was given.
#[derive(Debug, Clone)]
pub struct MemoryTurn {
    /// The config the session runs under.
    pub config: std::sync::Arc<Config>,
    /// Who the turn runs as.
    pub identity: ResolvedIdentity,
    /// The conversation thread.
    pub thread_id: String,
    /// The pack the turn was given, if any.
    pub pack: Option<TurnPack>,
}

/// What the agent loop knows before the model runs.
#[derive(Debug, Clone)]
pub struct PreTurnInput {
    /// The conversation thread.
    pub thread_id: String,
    /// The user message's index ([`user_turn_index`]).
    pub turn_index: u32,
    /// What the user said (without host-added context blocks).
    pub user_text: String,
    /// The first turn index still verbatim in the prompt; the pack leaves
    /// the thread from there on out.
    pub in_prompt_from: u32,
    /// When the user sent it.
    pub at: DateTime<Utc>,
    /// The session resumes a thread whose earlier turns were compacted out
    /// of the prompt: the pack opens with what the thread holds.
    pub resumed_after_compaction: bool,
}

/// Logs the user turn and recalls the pack it is given, within
/// `[memory.recall] pre_turn_timeout_ms`. `None` when memory is off, the
/// identity has neither logging nor recall, nothing relevant is stored, or
/// the engine is slow or failing — the turn runs either way.
///
/// The work is spawned, so a timed-out log still completes in the
/// background; only its pack is dropped.
pub async fn pre_turn(
    config: &Config,
    identity: &ResolvedIdentity,
    input: PreTurnInput,
) -> Option<TurnPack> {
    let logging = config.memory.conversations.enabled;
    if !logging && !identity.recall {
        return None;
    }
    let bound = match engine::resolve(config).engine() {
        Ok(bound) => bound,
        Err(error) => {
            tracing::debug!(%error, "[memory:hooks] pre_turn skipped: memory off");
            return None;
        }
    };
    let memory = match agent_memory_on(bound.engine, config, identity) {
        Ok(memory) => memory,
        Err(error) => {
            tracing::warn!(%error, "[memory:hooks] pre_turn skipped: no agent memory");
            return None;
        }
    };
    let memory = if identity.recall {
        memory
    } else {
        let quiet = log_only(memory.policy().clone());
        memory.with_policy(quiet)
    };
    let recall = identity.recall;
    let thread_id = input.thread_id.clone();
    let agent_id = identity.agent_id.clone();
    let started = std::time::Instant::now();
    let task = tokio::spawn(async move {
        let session = async {
            if recall && input.resumed_after_compaction {
                memory
                    .start_session(SessionStart {
                        thread_id: Some(input.thread_id.clone()),
                        focus: None,
                    })
                    .await
                    .map_err(|error| {
                        tracing::debug!(%error, "[memory:hooks] start_session failed");
                    })
                    .ok()
            } else {
                None
            }
        };
        let turn = async {
            if logging {
                let mut pre = PreTurn::new(&input.thread_id, input.turn_index, &input.user_text);
                pre.in_prompt_from = input.in_prompt_from;
                pre.at = Some(input.at);
                match memory.pre_turn(pre).await {
                    Ok(context) => {
                        if let Some(error) = &context.log_error {
                            tracing::warn!(
                                thread_id = %input.thread_id,
                                error = %error,
                                "[memory:hooks] user turn not logged"
                            );
                        }
                        Some(context.pack)
                    }
                    Err(error) => {
                        tracing::warn!(%error, "[memory:hooks] pre_turn refused");
                        None
                    }
                }
            } else {
                memory
                    .recall(&input.user_text)
                    .await
                    .map_err(|error| tracing::warn!(%error, "[memory:hooks] recall failed"))
                    .ok()
            }
        };
        let (session, turn) = futures::join!(session, turn);
        if recall {
            TurnPack::from_packs(session.into_iter().chain(turn))
        } else {
            None
        }
    });
    let timeout = Duration::from_millis(config.memory.recall.pre_turn_timeout_ms.max(1));
    let pack = match tokio::time::timeout(timeout, task).await {
        Ok(Ok(pack)) => pack,
        Ok(Err(error)) => {
            tracing::warn!(%error, "[memory:hooks] pre_turn task failed");
            None
        }
        Err(_) => {
            tracing::warn!(
                thread_id = %thread_id,
                timeout_ms = timeout.as_millis() as u64,
                "[memory:hooks] pre_turn timed out; the turn runs without a pack"
            );
            None
        }
    };
    if let Some(pack) = &pack {
        crate::memory::tools::record_pack_citations(&thread_id, pack.citations.clone());
    }
    tracing::debug!(
        thread_id = %thread_id,
        agent_id = %agent_id,
        injected = pack.is_some(),
        tokens = pack.as_ref().map_or(0, |pack| pack.tokens),
        refs = pack.as_ref().map_or(0, |pack| pack.refs.len()),
        elapsed_ms = started.elapsed().as_millis() as u64,
        "[memory:hooks] pre_turn"
    );
    pack
}

/// A tool call the reply made, with a one-line result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolCallSummary {
    /// The tool's name.
    pub name: String,
    /// The call id.
    pub id: Option<String>,
    /// The start of its result, if it produced one.
    pub result: Option<String>,
}

/// What the agent loop knows once a turn is committed.
#[derive(Debug, Clone)]
pub struct PostTurnInput {
    /// The conversation thread.
    pub thread_id: String,
    /// The reply's index: the user message's plus one.
    pub turn_index: u32,
    /// The final reply.
    pub assistant_text: String,
    /// Every tool call the turn made.
    pub tool_calls: Vec<ToolCallSummary>,
    /// When the reply was finished.
    pub at: DateTime<Utc>,
}

/// The reply as logged: its text, then one `tool → result` line per call,
/// so what a tool returned survives in memory even when the reply does not
/// repeat it.
#[must_use]
pub fn logged_reply(text: &str, calls: &[ToolCallSummary]) -> String {
    let lines: Vec<String> = calls
        .iter()
        .filter_map(|call| {
            let result = call
                .result
                .as_deref()?
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            (!result.is_empty()).then(|| {
                let result: String = result.chars().take(MAX_TOOL_LINE_CHARS).collect();
                format!("- {} → {result}", call.name)
            })
        })
        .collect();
    if lines.is_empty() {
        text.trim().to_string()
    } else {
        format!("{}\n\nTools:\n{}", text.trim(), lines.join("\n"))
    }
}

/// Logs a committed reply and queues the belief builds TinyMemory hands
/// back. Logs instead of failing.
pub async fn post_turn(config: &Config, identity: &ResolvedIdentity, input: PostTurnInput) {
    if !config.memory.conversations.enabled {
        return;
    }
    let text = logged_reply(&input.assistant_text, &input.tool_calls);
    if text.is_empty() {
        return;
    }
    let bound = match engine::resolve(config).engine() {
        Ok(bound) => bound,
        Err(_) => return,
    };
    let memory = match agent_memory_on(bound.engine, config, identity) {
        Ok(memory) => memory,
        Err(error) => {
            tracing::warn!(%error, "[memory:hooks] post_turn skipped");
            return;
        }
    };
    let mut post = PostTurn::new(&input.thread_id, input.turn_index, text);
    post.at = Some(input.at);
    post.tool_calls = input
        .tool_calls
        .iter()
        .map(|call| ToolCallRef {
            name: call.name.clone(),
            id: call.id.clone(),
        })
        .collect();
    match memory.post_turn(post).await {
        Ok(report) => {
            tracing::debug!(
                thread_id = %input.thread_id,
                agent_id = %identity.agent_id,
                turn = input.turn_index,
                jobs = report.jobs.len(),
                "[memory:hooks] reply logged"
            );
            jobs::enqueue(config, identity.root(), report.jobs).await;
        }
        Err(error) => {
            tracing::warn!(
                thread_id = %input.thread_id,
                %error,
                "[memory:hooks] reply not logged"
            );
        }
    }
}

/// Recalls what the compacted `dropped` turns carried, within
/// `[memory.recall] compaction_timeout_ms`. `None` when memory is off or
/// recall is off for the identity, or nothing comes back in time.
pub async fn compaction(
    config: &Config,
    identity: &ResolvedIdentity,
    thread_id: &str,
    dropped: Vec<Turn>,
) -> Option<TurnPack> {
    if !identity.recall || dropped.is_empty() || thread_id.trim().is_empty() {
        return None;
    }
    let bound = engine::resolve(config).engine().ok()?;
    let memory = agent_memory_on(bound.engine, config, identity).ok()?;
    let count = dropped.len();
    let request = Compaction {
        thread_id: thread_id.to_string(),
        dropped,
        focus: None,
    };
    let timeout = Duration::from_millis(config.memory.recall.compaction_timeout_ms.max(1));
    let pack = match tokio::time::timeout(timeout, memory.recall_for_compaction(request)).await {
        Ok(Ok(pack)) => TurnPack::from_packs([pack]),
        Ok(Err(error)) => {
            tracing::warn!(%error, "[memory:hooks] compaction recall failed");
            None
        }
        Err(_) => {
            tracing::warn!(thread_id, "[memory:hooks] compaction recall timed out");
            None
        }
    };
    tracing::debug!(
        thread_id,
        dropped = count,
        recalled = pack.is_some(),
        "[memory:hooks] compaction"
    );
    pack
}

#[cfg(test)]
#[path = "hooks_tests.rs"]
mod tests;
