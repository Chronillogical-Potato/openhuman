//! Tools *about* the tool surface itself.
//!
//! One member: [`deferred`], the host's half of
//! [`ToolExposure::Deferred`](tinytools::ToolExposure) — which registered tools
//! leave the wire so the harness can advertise its `tool_search` bridge in
//! their place. They sit in their own family rather than under `system/`
//! because it is not a capability the host offers the user — it is the model
//! asking what it is able to do.

pub mod deferred;

pub use deferred::{
    deferred_set, deferred_tool_names, strip_deferred_from_visible, TOOL_SEARCH_NAME,
};
