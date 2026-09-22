//! Tools *about* the tool surface itself.
//!
//! Two members: [`collapse`], the shared arithmetic behind a collapsed tool
//! (`memory`, `todo`, `delegate_to`), and [`deferred`], the host's half of
//! [`ToolExposure::Deferred`](tinytools::ToolExposure) — which registered tools
//! leave the wire so the harness can advertise its `tool_search` bridge in
//! their place. They sit in their own family rather than under `system/`
//! because neither is a capability the host offers the user — it is the model
//! asking what it is able to do.

pub mod collapse;
pub mod deferred;

pub use collapse::{
    any_external_effect, args_without_action, merge_action_schemas, resolve, strictest_permission,
    unknown_action_message, CollapsedAction,
};
pub use deferred::{deferred_tool_names, strip_deferred_from_visible, TOOL_SEARCH_NAME};
