//! What a user may call in SaaS mode.
//!
//! A user agent's context enables a few domain families
//! ([`host::user_domains`](super::host::user_domains)), but a family is too
//! coarse to open whole: `threads` also holds operations that start model
//! turns or reach the host. So in a user's scope every RPC method must also
//! be on [`USER_METHODS`], an explicit, reviewed allowlist. The registry
//! applies it at dispatch, in the controller list and in `/schema` alike
//! (`core::all`), so an unlisted method is indistinguishable from an absent
//! one.
//!
//! The operator plane and single-user processes are unaffected.

use crate::core::runtime::{is_saas, CoreContext};

/// Every RPC method a user may dispatch. Grows as each family's per-user
/// isolation lands; a method is listed only once nothing it touches is
/// shared between users.
pub const USER_METHODS: &[&str] = &[
    // Conversation threads: all state lives under the agent's workspace.
    "openhuman.threads_list",
    "openhuman.threads_upsert",
    "openhuman.threads_create_new",
    "openhuman.threads_messages_list",
    "openhuman.threads_message_append",
    "openhuman.threads_message_update",
    "openhuman.threads_update_labels",
    "openhuman.threads_update_title",
    "openhuman.threads_delete",
    "openhuman.threads_purge",
    "openhuman.threads_turn_state_get",
    "openhuman.threads_turn_state_list",
    "openhuman.threads_turn_state_history",
    "openhuman.threads_turn_state_get_turn",
    "openhuman.threads_turn_state_clear",
    "openhuman.threads_token_usage",
    "openhuman.threads_transcript_get",
    "openhuman.threads_goal_get",
    "openhuman.threads_todos_get",
];

/// Whether `method` (of an operator-plane controller or not) may be
/// dispatched or listed in the current scope.
pub fn method_visible(method: &str, operator_plane: bool) -> bool {
    visible_in(
        is_saas(),
        CoreContext::current()
            .as_deref()
            .and_then(CoreContext::session_agent)
            .is_some(),
        method,
        operator_plane,
    )
}

/// [`method_visible`] as a pure function of the mode and the scope.
///
/// In SaaS the two planes never overlap: the operator scope reaches only the
/// operator plane, and a user's scope only [`USER_METHODS`]. The SaaS
/// `DomainSet` registers the user families on the runtime so user contexts
/// can derive them; this keeps the operator from serving them on its own
/// workspace.
pub fn visible_in(saas: bool, user_scope: bool, method: &str, operator_plane: bool) -> bool {
    match (saas, user_scope) {
        (false, _) => true,
        (true, false) => operator_plane,
        (true, true) => !operator_plane && USER_METHODS.contains(&method),
    }
}

/// Thread ids a user may choose for themselves. Ids are only unique per agent,
/// but a few prefixes mean something to the core (channel conversations,
/// proactive jobs, sub-agent threads) and a user must not mint them.
pub fn validate_user_thread_id(id: &str) -> Result<(), String> {
    const RESERVED: &[&str] = &["channel:", "proactive:", "subagent:"];
    if id.is_empty() || id.len() > 128 {
        return Err("thread id must be 1 to 128 characters".to_string());
    }
    if RESERVED.iter().any(|prefix| id.starts_with(prefix)) {
        return Err(format!("thread id `{id}` uses a reserved prefix"));
    }
    if !id
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("thread id may contain only letters, digits, '-' and '_'".to_string());
    }
    Ok(())
}

/// Whether the current work runs for a SaaS user.
fn in_user_scope() -> bool {
    is_saas()
        && CoreContext::current()
            .as_deref()
            .and_then(CoreContext::session_agent)
            .is_some()
}

/// A SaaS user cannot pick a thread's working folder: their threads always
/// act in their own sandbox. Any folder is refused in user scope; outside it
/// nothing changes.
pub fn check_working_dir(action_dir: Option<&str>) -> Result<(), String> {
    match action_dir.map(str::trim).filter(|dir| !dir.is_empty()) {
        Some(_) if in_user_scope() => {
            Err("a thread's working folder cannot be chosen here".to_string())
        }
        _ => Ok(()),
    }
}

/// [`validate_user_thread_id`] when the current work runs for a SaaS user;
/// otherwise every id the single-user core accepts stays accepted.
pub fn check_thread_id(id: &str) -> Result<(), String> {
    if in_user_scope() {
        validate_user_thread_id(id)
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
