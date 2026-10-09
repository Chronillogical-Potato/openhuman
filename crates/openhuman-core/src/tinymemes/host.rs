//! One tinymemes engine per workspace, plus the state it keeps on disk under
//! `<workspace>/tinymemes/`:
//!
//! - `slang-index.json`: the slang index, which grows from web research.
//! - `remixed.json`: ids of thread messages that were delivered remixed, so the
//!   next reading can tell Jev which assistant turns carry the bot's own style.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex};

use tinymemes::{IndexPolicy, MemeEngine, SlangIndex, Turn};

use crate::threads::store::ConversationMessage;

const DIR: &str = "tinymemes";
const INDEX_FILE: &str = "slang-index.json";
const REMIXED_FILE: &str = "remixed.json";
/// Remixed-message ids remembered per workspace (oldest dropped first).
const REMIXED_CAP: usize = 4000;

pub(crate) const MODEL_ENV: &str = "OPENHUMAN_TINYMEMES_MODEL";
pub(crate) const KEY_ENV: &str = "OPENHUMAN_TINYMEMES_OPENROUTER_KEY";
const DEFAULT_MODEL: &str = "deepseek/deepseek-v4-flash";

pub(crate) struct Host {
    pub(crate) engine: MemeEngine,
    dir: PathBuf,
    remixed: Mutex<VecDeque<String>>,
}

static HOSTS: LazyLock<Mutex<HashMap<PathBuf, Arc<Host>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// The engine for a workspace, built on first use. `None` if it cannot be
/// built (logged once per attempt; the turn then goes out unchanged).
pub(crate) fn host_for(workspace_dir: &Path) -> Option<Arc<Host>> {
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

    let key = std::env::var(KEY_ENV)
        .ok()
        .filter(|k| !k.trim().is_empty())
        .unwrap_or_else(|| super::key::OPENROUTER_KEY.to_owned());
    let model = std::env::var(MODEL_ENV).unwrap_or_else(|_| DEFAULT_MODEL.to_owned());
    let engine = match MemeEngine::openrouter(&key, &model) {
        Ok(builder) => builder
            .slang_index(Arc::new(index))
            // Research runs in the background after delivery, never on the
            // reply's critical path.
            .learn_inline(None)
            .build(),
        Err(e) => {
            log::warn!("[tinymemes] engine build failed: {e}");
            return None;
        }
    };
    log::info!(
        "[tinymemes] engine ready model={model} slang_terms={}",
        engine.slang_index().len("IN")
    );
    let host = Arc::new(Host {
        engine,
        dir,
        remixed: Mutex::new(remixed),
    });
    hosts.insert(workspace_dir.to_path_buf(), host.clone());
    Some(host)
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
