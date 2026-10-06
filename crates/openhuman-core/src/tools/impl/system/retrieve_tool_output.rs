//! Tool: retrieve_tool_output — fetch the original of a compacted tool result.
//!
//! The tool itself lives in `tinyjuice::host::retrieve_tool`; OpenHuman only
//! supplies the lookup, which asks the TinyJuice module for the stashed
//! original by hash.

use std::sync::Arc;

pub use tinyjuice::host::retrieve_tool::RetrieveToolOutputTool;

/// The tool wired to the TokenJuice module's store.
pub fn retrieve_tool_output_tool() -> RetrieveToolOutputTool {
    RetrieveToolOutputTool::new(Arc::new(|hash| {
        Box::pin(async move { crate::inference::tokenjuice::retrieve(hash, None).await })
    }))
}

#[cfg(test)]
#[path = "retrieve_tool_output_tests.rs"]
mod tests;
