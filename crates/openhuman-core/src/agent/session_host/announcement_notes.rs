//! Mid-conversation availability notes, prepended to the user's next message
//! when an integration, MCP server or skill was connected or installed after
//! the session's system prompt was built (`runtime_session.rs` parks the ids
//! and `apply_pending_announcements` renders them).

// Availability notes prepended to the next user message when something was
// connected or installed mid-conversation. They are *status*, not a task:
// the earlier "act on them immediately" phrasing read as an instruction and
// sent the orchestrator off to the integrations agent in the middle of an
// unrelated exchange ("so lets do 20-30 days then?" → "What are we doing with
// the inbox?"). The user's message that follows is the only thing to act on.
const ANNOUNCEMENT_TRAILER: &str = "This is a capability update, not a request: \
respond to the user's message below and only use these if that message needs them. \
Do not tell the user to reconnect or restart.";

pub(super) fn integration_announcement_note(slugs: &[String]) -> Option<String> {
    (!slugs.is_empty()).then(|| format!(
        "[integration update] These integration(s) connected during this conversation and are available now via delegate_to_integrations_agent with the matching toolkit slug: {}. {ANNOUNCEMENT_TRAILER}",
        slugs.join(", ")
    ))
}

pub(super) fn mcp_announcement_note(servers: &[String]) -> Option<String> {
    (!servers.is_empty()).then(|| format!(
        "[MCP update] These MCP server(s) connected during this conversation and are available now via the use_mcp_server delegate: {}. {ANNOUNCEMENT_TRAILER}",
        servers.join(", ")
    ))
}

pub(super) fn skill_announcement_note(skill_ids: &[String]) -> Option<String> {
    (!skill_ids.is_empty()).then(|| format!(
        "[skills update] These skill(s) were installed during this conversation and are available now in your `## Installed Skills` list via `run_skill`: {}. {ANNOUNCEMENT_TRAILER}",
        skill_ids.join(", ")
    ))
}

pub(super) fn skill_retraction_note(skill_ids: &[String]) -> Option<String> {
    (!skill_ids.is_empty()).then(|| format!(
        "[skills retracted] These skill(s) were uninstalled during this conversation and are no longer available: {}. \
Do not attempt to run them with `run_skill` — they have been removed. Tell the user to reinstall if they want to use them again.",
        skill_ids.join(", ")
    ))
}
