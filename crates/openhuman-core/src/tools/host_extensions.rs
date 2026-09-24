//! OpenHuman-owned metadata recovered from TinyTools' erased extension seams.
//!
//! These helpers are intentionally separate from the `tinytools` vocabulary:
//! pack registries, delegation targets, and generated-tool runtime metadata are
//! host implementation details, not portable tool-trait APIs.

use crate::agent::orchestration::tools::DelegationTarget;
use crate::agent::tool_policy::GeneratedToolRuntimeContext;
use crate::tools::toolpacks::PackRegistryHandle;
#[cfg(test)]
use tinytools::{PermissionLevel, ToolCategory, ToolResult, ToolScope};
use tinytools::{Tool, ToolRunContext};

/// Reads a tool's pack-registry handle from its erased host extension.
pub fn pack_registry_handle(tool: &dyn Tool) -> Option<&PackRegistryHandle> {
    tool.host_extension()
        .and_then(|any| any.downcast_ref::<PackRegistryHandle>())
}

/// Reads the target agent a synthesized `delegate_*` tool routes to.
pub fn delegation_target(tool: &dyn Tool) -> Option<&str> {
    tool.host_extension()
        .and_then(|any| any.downcast_ref::<DelegationTarget>())
        .map(|target| target.0.as_str())
}

/// Reads generated-tool runtime metadata from the erased per-call extension.
pub fn generated_runtime_context(
    tool: &dyn Tool,
    args: &serde_json::Value,
) -> Option<GeneratedToolRuntimeContext> {
    tool.host_call_extension(args)
        .and_then(|any| any.downcast::<GeneratedToolRuntimeContext>().ok())
        .map(|boxed| *boxed)
}

/// Reads the provider-assigned tool-call id from a canonical tool's run
/// context, when the run is driven through the tinyagents harness.
///
/// `ToolRunContext`'s portable surface (workspace, thread id, output cap)
/// deliberately does not carry the call id — it is harness-owned. This
/// downcasts the erased host extension to tinyagents'
/// `ToolExecutionContext` (the same seam `ToolExecutionContext`'s own doc
/// comment documents) and reads `call_id` off it. `None` for a context that
/// carries no such extension (e.g. a test double) or no context at all.
pub fn tool_call_id(ctx: Option<&dyn ToolRunContext>) -> Option<String> {
    ctx.and_then(ToolRunContext::host_extension)
        .and_then(|any| any.downcast_ref::<tinyagents_harness::tool::ToolExecutionContext>())
        .map(|harness_ctx| harness_ctx.call_id.as_str().to_string())
}

#[cfg(test)]
#[path = "traits_tests.rs"]
mod tests;
