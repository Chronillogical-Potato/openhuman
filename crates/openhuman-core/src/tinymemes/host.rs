//! One tinymemes engine per workspace, plus the state it keeps on disk under
//! `<workspace>/tinymemes/`:
//!
//! - `slang-index.json`: the slang index, which grows from web research.
//! - `remixed.json`: ids of thread messages that were delivered remixed, so the
//!   next reading can tell Jev which assistant turns carry the bot's own style.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use tinymemes::{
    ChatModel, EnvConfig, Evaluator, IndexPolicy, MemeEngine, RatingPolicy, SearchResearcher,
    SlangIndex, SlangResearcher, Turn,
};

use crate::config::Config;

use crate::threads::store::ConversationMessage;

const DIR: &str = "tinymemes";
const INDEX_FILE: &str = "slang-index.json";
const MEME_INDEX_FILE: &str = "meme-index.json";
/// Newly learned GIFs are pending until a person approves them; `0` auto-approves
/// GIFs that pass every check instead.
pub(crate) const MEME_REVIEW_ENV: &str = "OPENHUMAN_TINYMEMES_MEME_REVIEW";
const REMIXED_FILE: &str = "remixed.json";
/// Remixed-message ids remembered per workspace (oldest dropped first).
const REMIXED_CAP: usize = 4000;

/// Replies to wait after a meme before sending another (0 = no cooldown).
pub(crate) const MEME_COOLDOWN_ENV: &str = "OPENHUMAN_TINYMEMES_MEME_COOLDOWN";

pub(crate) struct Host {
    pub(crate) engine: MemeEngine,
    dir: PathBuf,
    remixed: Mutex<VecDeque<String>>,
}

static HOSTS: LazyLock<Mutex<HashMap<PathBuf, Arc<Host>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The engine for a workspace, built on first use. `None` if it cannot be
/// built (the turn then goes out unchanged).
///
/// Backends, each overridable by `TINYMEMES_*` env (see `tinymemes::env`):
///
/// | piece | default (OpenHuman) | env override |
/// | --- | --- | --- |
/// | chat model | OpenHuman's `summarization` provider | `TINYMEMES_OPENROUTER_KEY` (+ `TINYMEMES_MODEL`) |
/// | Jev | OpenHuman-managed; the LLM fallback when unavailable | `TINYMEMES_JEV` (+ its key variables) |
/// | slang research | OpenHuman web search + the chat model | `TINYMEMES_OPENROUTER_KEY` (OpenRouter web plugin) |
pub(crate) fn host_for(config: &Config) -> Option<Arc<Host>> {
    let workspace_dir = config.workspace_dir.as_path();
    let mut hosts = HOSTS.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(host) = hosts.get(workspace_dir) {
        return Some(host.clone());
    }
    let dir = workspace_dir.join(DIR);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("[tinymemes] cannot create state dir: {e}");
        return None;
    }
    let index = match std::fs::read_to_string(dir.join(INDEX_FILE)) {
        Ok(json) => SlangIndex::from_json(&json, IndexPolicy::default()).unwrap_or_else(|e| {
            log::warn!("[tinymemes] slang index unreadable, starting fresh: {e}");
            SlangIndex::new(IndexPolicy::default())
        }),
        Err(_) => SlangIndex::new(IndexPolicy::default()),
    };
    let remixed: VecDeque<String> = std::fs::read_to_string(dir.join(REMIXED_FILE))
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();

    let env = EnvConfig::from_env();
    let http = reqwest::Client::new();

    let (chat, chat_label): (Arc<dyn ChatModel>, String) = match env.chat_model(&http) {
        Some(model) => (model, format!("env:openrouter/{}", env.model_id())),
        None => (
            Arc::new(super::inference::OpenHumanChatModel::new(config.clone())),
            format!("openhuman:{}", super::inference::resolved_model(config)),
        ),
    };

    // Jev: OpenHuman-managed always, unless TINYMEMES_JEV overrides it. With
    // no managed Jev (signed out / offline session), OpenHuman's LLM answers
    // Jev's questions instead.
    let (jev, jev_label): (Arc<dyn Evaluator>, &str) = if env.forces_llm_jev() {
        (
            tinymemes::env::llm_jev(chat.clone()),
            "llm (TINYMEMES_JEV=llm)",
        )
    } else {
        let overridden = match env.jev {
            Some(_) => env.jev().unwrap_or_else(|e| {
                log::warn!("[tinymemes] TINYMEMES_JEV override unusable, using managed: {e}");
                None
            }),
            None => None,
        };
        match overridden {
            Some((jev, label)) => (jev, label),
            None => match super::jev::managed(config) {
                Some(jev) => (jev, "openhuman-managed"),
                None => (
                    tinymemes::env::llm_jev(chat.clone()),
                    "llm (managed jev unavailable)",
                ),
            },
        }
    };

    let search = openhuman_search(config);
    let meme_policy = tinymemes::MemeIndexPolicy {
        auto_approve: std::env::var(MEME_REVIEW_ENV).is_ok_and(|v| v.trim() == "0"),
        ..tinymemes::MemeIndexPolicy::default()
    };
    let meme_index = match std::fs::read_to_string(dir.join(MEME_INDEX_FILE)) {
        Ok(json) => tinymemes::MemeIndex::from_json(&json, meme_policy).unwrap_or_else(|e| {
            log::warn!("[tinymemes] meme index unreadable, starting fresh: {e}");
            tinymemes::MemeIndex::new(meme_policy)
        }),
        Err(_) => tinymemes::MemeIndex::new(meme_policy),
    };
    let (researcher, research_label): (Option<Arc<dyn SlangResearcher>>, &str) =
        match env.web_researcher(&http) {
            Some(r) => (Some(Arc::new(r)), "env:openrouter-web"),
            None => match search.clone() {
                Some(search) => (
                    Some(Arc::new(SearchResearcher::new(search, chat.clone()))),
                    "openhuman-search",
                ),
                None => (None, "off (no search provider)"),
            },
        };

    let mut builder = MemeEngine::builder(jev, chat)
        .source(Arc::new(tinymemes::source::Imgflip::new(http.clone())))
        .slang_index(Arc::new(index))
        .meme_index(Arc::new(meme_index))
        .policy(rating_policy())
        // Research runs in the background after delivery, never on the
        // reply's critical path.
        .learn_inline(None);
    if let Some(researcher) = researcher {
        builder = builder.researcher(researcher);
    }
    let meme_research_label = match search {
        Some(search) => {
            builder = builder
                .meme_researcher(Arc::new(tinymemes::GiphyPageResearcher::new(search, http)));
            if meme_policy.auto_approve {
                "giphy-pages"
            } else {
                "giphy-pages (review)"
            }
        }
        None => "off (no search provider)",
    };
    let engine = builder.build();
    log::info!(
        "[tinymemes] engine ready chat={chat_label} jev={jev_label} research={research_label} \
         meme_research={meme_research_label} slang_terms={} learned_memes={}",
        engine.slang_index().len("IN"),
        engine.meme_index().len("IN")
    );
    let host = Arc::new(Host {
        engine,
        dir,
        remixed: Mutex::new(remixed),
    });
    hosts.insert(workspace_dir.to_path_buf(), host.clone());
    Some(host)
}

#[cfg(feature = "modules")]
fn openhuman_search(config: &Config) -> Option<Arc<dyn tinymemes::WebSearch>> {
    super::search::OpenHumanSearch::available(config)
        .map(|s| Arc::new(s) as Arc<dyn tinymemes::WebSearch>)
}

#[cfg(not(feature = "modules"))]
fn openhuman_search(_config: &Config) -> Option<Arc<dyn tinymemes::WebSearch>> {
    None
}

fn rating_policy() -> RatingPolicy {
    let mut policy = RatingPolicy::default();
    if let Some(turns) = std::env::var(MEME_COOLDOWN_ENV)
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        policy.meme_cooldown_turns = turns;
    }
    policy
}

impl Host {
    pub(crate) fn is_remixed(&self, message_id: &str) -> bool {
        self.remixed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .any(|id| id == message_id)
    }

    pub(crate) fn mark_remixed(&self, message_id: String) {
        let snapshot = {
            let mut ids = self.remixed.lock().unwrap_or_else(|e| e.into_inner());
            ids.push_back(message_id);
            while ids.len() > REMIXED_CAP {
                ids.pop_front();
            }
            serde_json::to_string(&*ids).unwrap_or_default()
        };
        if let Err(e) = std::fs::write(self.dir.join(REMIXED_FILE), snapshot) {
            log::warn!("[tinymemes] cannot save remixed ids: {e}");
        }
    }

    pub(crate) fn save_memes(&self) {
        let json = self.engine.meme_index().to_json();
        if let Err(e) = std::fs::write(self.dir.join(MEME_INDEX_FILE), json) {
            log::warn!("[tinymemes] cannot save meme index: {e}");
        }
    }

    pub(crate) fn save_index(&self) {
        let json = self.engine.slang_index().to_json();
        if let Err(e) = std::fs::write(self.dir.join(INDEX_FILE), json) {
            log::warn!("[tinymemes] cannot save slang index: {e}");
        }
    }
}

/// The thread as the user saw it, oldest first: delivered replies (remixed or
/// not) and the user's messages. `current_user_message` is appended when the
/// store does not have it yet.
pub(crate) fn history_turns(
    messages: &[ConversationMessage],
    current_user_message: &str,
    is_remixed: impl Fn(&str) -> bool,
) -> Vec<Turn> {
    let mut turns: Vec<Turn> = messages
        .iter()
        .filter(|m| !m.content.trim().is_empty())
        .filter_map(|m| match m.sender.as_str() {
            "user" => Some(Turn::user(m.content.clone())),
            "agent" | "assistant" => Some(if is_remixed(&m.id) {
                Turn::remixed(m.content.clone())
            } else {
                Turn::assistant(m.content.clone())
            }),
            _ => None,
        })
        .collect();
    let current = current_user_message.trim();
    let already_there = turns
        .iter()
        .rev()
        .find(|t| t.role == tinymemes::Role::User)
        .is_some_and(|t| t.text.trim() == current);
    if !current.is_empty() && !already_there {
        turns.push(Turn::user(current.to_owned()));
    }
    turns
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
