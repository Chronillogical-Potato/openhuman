//! One tinymemes engine per workspace, plus the state it keeps on disk under
//! `<workspace>/tinymemes/`:
//!
//! - `slang-index.json`: the slang index, which grows from web research.
//! - `meme-index.json`: reaction GIFs learned from the web, already vetted.
//! - `remixed.json`: ids of thread messages that were delivered remixed, so the
//!   next reading can tell Jev which assistant turns carry the bot's own style.
//!
//! Files are written atomically (temp file, then rename). The engine is
//! rebuilt when the configuration or credentials it was built from change,
//! keeping its learned state.

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
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
const REMIXED_FILE: &str = "remixed.json";
/// Remixed-message ids remembered per workspace (oldest dropped first).
const REMIXED_CAP: usize = 4000;

/// Replies to wait after a meme before sending another (0 = no cooldown).
pub(crate) const MEME_COOLDOWN_ENV: &str = "OPENHUMAN_TINYMEMES_MEME_COOLDOWN";

pub(crate) struct Host {
    pub(crate) engine: MemeEngine,
    dir: PathBuf,
    remixed: Mutex<VecDeque<String>>,
    /// What the engine was built from; a change rebuilds it.
    fingerprint: u64,
}

/// `TINYMEMES_*` overrides that change how the engine is built.
const ENV_KEYS: &[&str] = &[
    "TINYMEMES_OPENROUTER_KEY",
    "TINYMEMES_MODEL",
    "TINYMEMES_JEV",
    "TINYMEMES_TYPESAFE_KEY",
    "TYPESAFE_API_KEY",
    "TINYMEMES_OPENJEV_KEY",
    "OPENJEV_API_KEY",
    MEME_COOLDOWN_ENV,
];

/// A fingerprint of everything the engine is built from: the provider route,
/// endpoints, the backend credential (hashed in memory, never stored or
/// logged), search availability, and the env overrides. Settings changes,
/// sign-in after first use, and credential rotation all change it.
pub(crate) fn fingerprint(config: &Config) -> u64 {
    let mut h = DefaultHasher::new();
    config.memory_provider.hash(&mut h);
    config.inference_url.hash(&mut h);
    config.api_url.hash(&mut h);
    config.default_model.hash(&mut h);
    crate::security::credentials::session_support::resolve_backend_credential(config)
        .ok()
        .map(|c| c.into_secret())
        .hash(&mut h);
    openhuman_search(config).is_some().hash(&mut h);
    for key in ENV_KEYS {
        std::env::var(key).ok().hash(&mut h);
    }
    h.finish()
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
    let fp = fingerprint(config);
    let mut hosts = HOSTS.lock().unwrap_or_else(|e| e.into_inner());
    let previous = match hosts.get(workspace_dir) {
        Some(host) if host.fingerprint == fp => return Some(host.clone()),
        Some(host) => {
            log::info!("[tinymemes] configuration or credentials changed; rebuilding the engine");
            Some(host.clone())
        }
        None => None,
    };
    let dir = workspace_dir.join(DIR);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        log::warn!("[tinymemes] cannot create state dir: {e}");
        return None;
    }
    // Learned state carries over a rebuild; on first use it loads from disk.
    let (index, meme_index, remixed) = match &previous {
        Some(p) => (
            p.engine.slang_index().clone(),
            p.engine.meme_index().clone(),
            p.remixed.lock().unwrap_or_else(|e| e.into_inner()).clone(),
        ),
        None => (
            Arc::new(load_slang_index(&dir)),
            Arc::new(load_meme_index(&dir)),
            load_remixed(&dir),
        ),
    };

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
        .slang_index(index)
        .meme_index(meme_index)
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
            "giphy-pages"
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
    let host = Arc::new(Host::new(engine, dir, remixed, fp));
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

fn load_slang_index(dir: &Path) -> SlangIndex {
    match std::fs::read_to_string(dir.join(INDEX_FILE)) {
        Ok(json) => SlangIndex::from_json(&json, IndexPolicy::default()).unwrap_or_else(|e| {
            log::warn!("[tinymemes] slang index unreadable, starting fresh: {e}");
            SlangIndex::new(IndexPolicy::default())
        }),
        Err(_) => SlangIndex::new(IndexPolicy::default()),
    }
}

fn load_meme_index(dir: &Path) -> tinymemes::MemeIndex {
    let policy = tinymemes::MemeIndexPolicy::default();
    match std::fs::read_to_string(dir.join(MEME_INDEX_FILE)) {
        Ok(json) => tinymemes::MemeIndex::from_json(&json, policy).unwrap_or_else(|e| {
            log::warn!("[tinymemes] meme index unreadable, starting fresh: {e}");
            tinymemes::MemeIndex::new(policy)
        }),
        Err(_) => tinymemes::MemeIndex::new(policy),
    }
}

/// Remixed ids from disk, newest `REMIXED_CAP` kept.
fn load_remixed(dir: &Path) -> VecDeque<String> {
    let mut ids: VecDeque<String> = std::fs::read_to_string(dir.join(REMIXED_FILE))
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    cap_remixed(&mut ids);
    ids
}

fn cap_remixed(ids: &mut VecDeque<String>) {
    while ids.len() > REMIXED_CAP {
        ids.pop_front();
    }
}

static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// Write `contents` to `path` atomically: a unique temp file in the same
/// directory, then a rename over the target. Concurrent writers each rename a
/// complete file; a crash never leaves a half-written one.
pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let seq = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("state");
    let tmp = path.with_file_name(format!(".{name}.{}.{seq}.tmp", std::process::id()));
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
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
    /// A host over `engine`, keeping its state files in `dir`.
    pub(crate) fn new(
        engine: MemeEngine,
        dir: PathBuf,
        remixed: VecDeque<String>,
        fingerprint: u64,
    ) -> Self {
        Self {
            engine,
            dir,
            remixed: Mutex::new(remixed),
            fingerprint,
        }
    }

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
            if ids.contains(&message_id) {
                return;
            }
            ids.push_back(message_id);
            cap_remixed(&mut ids);
            serde_json::to_string(&*ids).unwrap_or_default()
        };
        if let Err(e) = write_atomic(&self.dir.join(REMIXED_FILE), &snapshot) {
            log::warn!("[tinymemes] cannot save remixed ids: {e}");
        }
    }

    pub(crate) fn save_memes(&self) {
        let json = self.engine.meme_index().to_json();
        if let Err(e) = write_atomic(&self.dir.join(MEME_INDEX_FILE), &json) {
            log::warn!("[tinymemes] cannot save meme index: {e}");
        }
    }

    pub(crate) fn save_index(&self) {
        let json = self.engine.slang_index().to_json();
        if let Err(e) = write_atomic(&self.dir.join(INDEX_FILE), &json) {
            log::warn!("[tinymemes] cannot save slang index: {e}");
        }
    }
}

/// The thread as the user saw it, oldest first: delivered replies (remixed or
/// not) and the user's messages. `current_user_message` is appended unless it
/// is already the last stored turn (the store may have persisted it first).
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
        .last()
        .is_some_and(|t| t.role == tinymemes::Role::User && t.text.trim() == current);
    if !current.is_empty() && !already_there {
        turns.push(Turn::user(current.to_owned()));
    }
    turns
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
