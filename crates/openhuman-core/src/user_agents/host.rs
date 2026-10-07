//! [`AgentHost`]: the user agents a SaaS process has open.
//!
//! A user agent is opened lazily on first use and kept until it has been idle
//! for [`SaasConfig::idle_evict_secs`] or the host needs its slot
//! ([`SaasConfig::max_agents_open`]). An agent still in use — anyone holding
//! its [`UserAgentState`] — is never evicted.
//!
//! Each open agent carries its own [`CoreContext`], derived from the operator
//! context with the agent's forced config and `session_agent` set to its id.
//! Running work under that context is what makes the session store, the
//! config loader and the web_chat session cache resolve that user's state.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::layout::{self, UserAgentLayout};
use super::types::{UserAgentId, UserAgentMeta, UserAgentSummary, LAYOUT_VERSION};
use crate::config::Config;
use crate::core::runtime::{ContextOverlay, CoreContext, DomainSet, SaasConfig};
use crate::tools::toolpacks::ToolGroups;

/// One open user agent.
pub struct UserAgentState {
    pub id: UserAgentId,
    pub layout: UserAgentLayout,
    pub config: Config,
    context: Arc<CoreContext>,
}

impl std::fmt::Debug for UserAgentState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserAgentState")
            .field("id", &self.id)
            .finish()
    }
}

impl UserAgentState {
    /// The context every piece of this agent's work runs under.
    pub fn context(&self) -> &Arc<CoreContext> {
        &self.context
    }
}

struct Slot {
    state: Arc<UserAgentState>,
    last_used: Instant,
}

/// The open user agents of one SaaS process.
pub struct AgentHost {
    saas: SaasConfig,
    operator: Arc<CoreContext>,
    open: Mutex<HashMap<UserAgentId, Slot>>,
}

impl std::fmt::Debug for AgentHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHost")
            .field("root", &self.saas.root)
            .field("open", &self.open_count())
            .finish()
    }
}

/// The domain families a user agent's context serves. None yet: each family
/// opens once its per-user isolation lands.
pub fn user_domains() -> DomainSet {
    DomainSet::none()
}

impl AgentHost {
    pub fn new(saas: SaasConfig, operator: Arc<CoreContext>) -> Self {
        Self {
            saas,
            operator,
            open: Mutex::new(HashMap::new()),
        }
    }

    fn layout(&self, id: &UserAgentId) -> UserAgentLayout {
        UserAgentLayout::new(&self.saas.root, id)
    }

    /// Create agent `id`'s directories. Returns whether it was new.
    pub fn provision(&self, id: &UserAgentId) -> Result<bool, String> {
        let layout = self.layout(id);
        if layout.meta_path.exists() {
            log::debug!("[user_agents] provision agent={id}: already provisioned");
            return Ok(false);
        }
        for dir in [&layout.workspace_dir, &layout.sandbox_dir] {
            std::fs::create_dir_all(dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
        }
        let meta = UserAgentMeta {
            agent_id: id.clone(),
            created_at: unix_now(),
            layout_version: LAYOUT_VERSION,
        };
        let raw = toml::to_string(&meta).map_err(|e| format!("encoding agent meta: {e}"))?;
        std::fs::write(&layout.meta_path, raw)
            .map_err(|e| format!("writing {}: {e}", layout.meta_path.display()))?;
        log::info!("[user_agents] provisioned agent={id}");
        Ok(true)
    }

    /// Close agent `id` and archive its state under `<root>/deprovisioned/`.
    /// Nothing is deleted. Returns whether there was such an agent.
    pub fn deprovision(&self, id: &UserAgentId) -> Result<bool, String> {
        self.lock().remove(id);
        let layout = self.layout(id);
        if !layout.dir.exists() {
            return Ok(false);
        }
        let archive = layout::archive_dir(&self.saas.root);
        std::fs::create_dir_all(&archive)
            .map_err(|e| format!("creating {}: {e}", archive.display()))?;
        let dest = archive.join(format!("{id}-{}", unix_now()));
        std::fs::rename(&layout.dir, &dest)
            .map_err(|e| format!("archiving {}: {e}", layout.dir.display()))?;
        log::info!("[user_agents] deprovisioned agent={id} (archived)");
        Ok(true)
    }

    /// Agent `id`, opening it if it is provisioned and not open yet.
    pub fn open(&self, id: &UserAgentId) -> Result<Arc<UserAgentState>, String> {
        let now = Instant::now();
        let mut open = self.lock();
        if let Some(slot) = open.get_mut(id) {
            slot.last_used = now;
            return Ok(Arc::clone(&slot.state));
        }

        let layout = self.layout(id);
        if !layout.meta_path.exists() {
            return Err(format!("agent {id} is not provisioned"));
        }
        self.evict_locked(&mut open, now);
        if open.len() >= self.saas.max_agents_open.max(1) {
            return Err(format!(
                "all {} agent slots are in use; try again shortly",
                self.saas.max_agents_open
            ));
        }

        let config = layout::agent_config(&layout, id);
        let context = self.operator.derive_with(
            ContextOverlay::new(config.clone(), user_domains(), ToolGroups::none())
                .without_user_skill_roots()
                .session_agent(id.as_str()),
        );
        let state = Arc::new(UserAgentState {
            id: id.clone(),
            layout,
            config,
            context,
        });
        open.insert(
            id.clone(),
            Slot {
                state: Arc::clone(&state),
                last_used: now,
            },
        );
        log::debug!("[user_agents] opened agent={id} ({} open)", open.len());
        Ok(state)
    }

    /// Agent `id` if it is open.
    pub fn get(&self, id: &UserAgentId) -> Option<Arc<UserAgentState>> {
        self.lock().get(id).map(|slot| Arc::clone(&slot.state))
    }

    pub fn is_open(&self, id: &UserAgentId) -> bool {
        self.lock().contains_key(id)
    }

    pub fn open_count(&self) -> usize {
        self.lock().len()
    }

    /// Every provisioned agent, open or not.
    pub fn list(&self) -> Result<Vec<UserAgentSummary>, String> {
        let dir = layout::agents_dir(&self.saas.root);
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("reading {}: {e}", dir.display())),
        };
        let mut found = Vec::new();
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Ok(id) = UserAgentId::parse(&name) else {
                continue;
            };
            if let Some(summary) = self.summary(&id)? {
                found.push(summary);
            }
        }
        found.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        Ok(found)
    }

    /// Agent `id`, or `None` when it is not provisioned.
    pub fn summary(&self, id: &UserAgentId) -> Result<Option<UserAgentSummary>, String> {
        let layout = self.layout(id);
        let raw = match std::fs::read_to_string(&layout.meta_path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(format!("reading {}: {e}", layout.meta_path.display())),
        };
        let meta: UserAgentMeta = toml::from_str(&raw)
            .map_err(|e| format!("parsing {}: {e}", layout.meta_path.display()))?;
        Ok(Some(UserAgentSummary {
            agent_id: id.clone(),
            created_at: meta.created_at,
            open: self.is_open(id),
        }))
    }

    /// Close agents idle past the configured limit that nobody holds.
    pub fn evict_idle(&self) {
        let mut open = self.lock();
        self.evict_locked(&mut open, Instant::now());
    }

    fn evict_locked(&self, open: &mut HashMap<UserAgentId, Slot>, now: Instant) {
        let idle_limit = Duration::from_secs(self.saas.idle_evict_secs);
        let in_use = |slot: &Slot| Arc::strong_count(&slot.state) > 1;
        open.retain(|id, slot| {
            let keep = in_use(slot) || now.duration_since(slot.last_used) < idle_limit;
            if !keep {
                log::debug!("[user_agents] evicted idle agent={id}");
            }
            keep
        });
        // Still full: make room by closing the least recently used idle one.
        if open.len() >= self.saas.max_agents_open.max(1) {
            let victim = open
                .iter()
                .filter(|(_, slot)| !in_use(slot))
                .min_by_key(|(_, slot)| slot.last_used)
                .map(|(id, _)| id.clone());
            if let Some(id) = victim {
                open.remove(&id);
                log::debug!("[user_agents] evicted least recently used agent={id}");
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<UserAgentId, Slot>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

static HOST: OnceLock<Arc<AgentHost>> = OnceLock::new();

/// Install the process's agent host. A SaaS boot does this once; later calls
/// are ignored.
pub fn install(host: Arc<AgentHost>) {
    if HOST.set(host).is_err() {
        log::warn!("[user_agents] agent host already installed; keeping the first");
    }
}

/// The process's agent host, when this is a SaaS process.
pub fn host() -> Option<Arc<AgentHost>> {
    HOST.get().cloned()
}

/// The user agent the current work runs for, if any.
pub fn current() -> Option<Arc<UserAgentState>> {
    let agent = CoreContext::current()?.session_agent()?.to_owned();
    let id = UserAgentId::parse(&agent).ok()?;
    host()?.get(&id)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
