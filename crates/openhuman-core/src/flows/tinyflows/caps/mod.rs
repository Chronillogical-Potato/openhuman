//! OpenHuman adapters for the `tinyflows::caps` host seam.
//!
//! This module stays export-focused. Capability construction, curation,
//! preflight, and invocation logic live in [`ops`]; individual trait adapters
//! live in their focused sibling modules.

mod agent;
mod code;
mod http;
mod llm;
mod ops;
mod prompt;
mod resolver;
mod tier;
pub(crate) mod tools;

// Preserve the existing `caps::X` paths used by flows and adapter siblings.
pub(crate) use agent::*;
pub(crate) use code::*;
pub(crate) use http::*;
pub(crate) use llm::*;
// The schema-aware dry-run doubles are `tinyflows::caps::mock_schema_aware`'s:
// "answer the shape this node declared" is a fact about the engine's own
// output-parser sub-port, not about OpenHuman. Re-exported under the historical
// `caps::` path so existing call sites resolve unchanged.
pub use ops::*;
pub(crate) use prompt::*;
// The pure prompt/reply helpers live upstream, beside the agent request builder.
pub(crate) use resolver::*;
pub(crate) use tier::*;
pub(crate) use tinyflows::caps::mock_schema_aware::*;
pub(crate) use tinyflows::nodes::integration::agent_prompt::{
    build_agent_result, build_harness_run_prompt, clamp_run_timeout_secs, extract_structured_json,
    prepend_system_message, resolve_node_model, resolve_run_timeout_secs,
    structured_output_requested,
};
pub(crate) use tools::NATIVE_TOOL_PREFIX;
