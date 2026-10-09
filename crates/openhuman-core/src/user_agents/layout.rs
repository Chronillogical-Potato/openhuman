//! Where one user agent's state lives, and the config it always runs with.
//!
//! ```text
//! <root>/agents/<agent-id>/
//!   agent.toml        UserAgentMeta
//!   config.toml       the agent's config_path
//!   workspace/        sessions, memory, threads, cron, cost — internal state
//!   sandbox/          the agent's action_dir: the only place it may act
//! <root>/deprovisioned/<agent-id>-<unix-secs>/   an archived agent
//! ```
//!
//! Every per-agent path sits under its own directory, so each store keyed by
//! workspace (cron, approvals, threads, the cost ledger, the session store) is
//! already separate per user without that store knowing about users.

use std::path::{Path, PathBuf};

use super::types::UserAgentId;
use crate::config::Config;
use crate::security::AutonomyLevel;

/// Resolved paths of one user agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserAgentLayout {
    pub dir: PathBuf,
    pub meta_path: PathBuf,
    pub config_path: PathBuf,
    pub workspace_dir: PathBuf,
    pub sandbox_dir: PathBuf,
}

impl UserAgentLayout {
    pub fn new(saas_root: &Path, id: &UserAgentId) -> Self {
        let dir = agents_dir(saas_root).join(id.as_str());
        Self {
            meta_path: dir.join("agent.toml"),
            config_path: dir.join("config.toml"),
            workspace_dir: dir.join("workspace"),
            sandbox_dir: dir.join("sandbox"),
            dir,
        }
    }
}

/// `<root>/agents`.
pub fn agents_dir(saas_root: &Path) -> PathBuf {
    saas_root.join("agents")
}

/// `<root>/deprovisioned`.
pub fn archive_dir(saas_root: &Path) -> PathBuf {
    saas_root.join("deprovisioned")
}

/// The memory namespace root of agent `id`.
pub fn memory_root(id: &UserAgentId) -> String {
    format!("user:{id}")
}

/// The config agent `id` runs with.
///
/// Everything that decides **where** the agent reads and writes, and **what
/// it may do**, is forced here and cannot come from anywhere else:
///
/// - every path sits under the agent's own directory;
/// - memory is bound to the agent (`[memory] agent_id`, `root = user:<id>`),
///   so a definition pin or a team root cannot move it onto another user's
///   tree (a host binding wins over both);
/// - the autonomy policy is on and supervised, with no auto-approval, no tool
///   installation, no trusted roots beyond the sandbox, and workspace-only
///   paths.
pub fn agent_config(layout: &UserAgentLayout, id: &UserAgentId) -> Config {
    let mut config = Config {
        config_path: layout.config_path.clone(),
        workspace_dir: layout.workspace_dir.clone(),
        action_dir: layout.sandbox_dir.clone(),
        ..Config::default()
    };
    // Artifacts land in the agent's sandbox, never the host's shared
    // `~/OpenHuman/projects/Files`.
    config.files_dir_override = Some(layout.sandbox_dir.join("files"));
    config.memory.agent_id = Some(id.to_string());
    config.memory.root = Some(memory_root(id));

    let autonomy = &mut config.autonomy;
    autonomy.enabled = true;
    autonomy.level = AutonomyLevel::Supervised;
    autonomy.workspace_only = true;
    autonomy.auto_approve_all = false;
    autonomy.allow_tool_install = false;
    autonomy.trusted_roots = Vec::new();
    config
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
