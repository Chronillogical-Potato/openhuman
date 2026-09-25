//! Subconscious engine selection. The local graph is the supported engine.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which engine runs the subconscious reflect/commit cognition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubconsciousEngine {
    /// The local tinyagents subconscious graph.
    /// Accept the retired name when loading old configuration, then persist
    /// the supported value on the next save.
    #[default]
    #[serde(alias = "medulla")]
    Local,
}

/// The `[subconscious]` config block.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SubconsciousConfig {
    /// Which engine drives the subconscious tick. Default `local`.
    #[serde(default)]
    pub engine: SubconsciousEngine,
}

#[cfg(test)]
#[path = "subconscious_tests.rs"]
mod tests;
