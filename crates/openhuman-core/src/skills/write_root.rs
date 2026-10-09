//! Where an embedded agent's own skills live.
//!
//! An agent derived on a shared workspace keeps the skills it installs or
//! creates under `<workspace>/agents/<id>/`, in place of the operator's home
//! directory: `skills/` for SKILL.md bundles, `workflows/` for automations.
//! Discovery scans those roots for that agent only, so a skill one agent
//! writes is neither seen by its siblings nor written into the operator's
//! `~/.openhuman`.

use std::path::{Path, PathBuf};

/// The current agent's skills home under `workspace_dir`, or `None` outside
/// an embedded agent's context.
#[must_use]
pub fn agent_skill_home(workspace_dir: &Path) -> Option<PathBuf> {
    let agent = crate::core::runtime::agent_scope::current_agent_id()?;
    Some(workspace_dir.join("agents").join(agent))
}

/// The current agent's skills home under its context's workspace, or `None`
/// outside an embedded agent's context — for callers that hold no workspace
/// path.
#[must_use]
pub fn current_agent_skill_home() -> Option<PathBuf> {
    let workspace = crate::core::runtime::CoreContext::current()?
        .workspace_dir()
        .ok()?;
    agent_skill_home(&workspace)
}

/// The roots an agent's user-scope bundles are discovered from and removed
/// from, newest layout last so it wins a name collision.
#[must_use]
pub fn agent_user_roots(agent_home: &Path) -> [PathBuf; 2] {
    [agent_home.join("skills"), agent_home.join("workflows")]
}

/// Where a user-scope SKILL.md bundle is installed: the agent's `skills/`
/// under an embedded agent's context, else `~/.openhuman/skills`.
#[must_use]
pub fn user_skill_install_root(workspace_dir: &Path, home: Option<&Path>) -> Option<PathBuf> {
    match agent_skill_home(workspace_dir) {
        Some(agent_home) => Some(agent_home.join("skills")),
        None => home.map(|home| home.join(".openhuman").join("skills")),
    }
}

/// Where a user-scope workflow is created: the agent's `workflows/` under an
/// embedded agent's context, else `~/.openhuman/workflows`.
#[must_use]
pub fn user_workflow_root(workspace_dir: &Path, home: Option<&Path>) -> Option<PathBuf> {
    match agent_skill_home(workspace_dir) {
        Some(agent_home) => Some(agent_home.join("workflows")),
        None => home.map(|home| home.join(".openhuman").join("workflows")),
    }
}

#[cfg(test)]
#[path = "write_root_tests.rs"]
mod tests;
