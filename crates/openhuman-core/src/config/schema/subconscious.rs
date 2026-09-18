//! Subconscious engine selection.
//!
//! The local tinyagents graph drives the heartbeat tick.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Which engine runs the subconscious cognition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SubconsciousEngine {
    /// The local tinyagents subconscious graph.
    #[default]
    Local,
}

/// The subconscious config block.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SubconsciousConfig {
    /// Which engine drives the subconscious tick.
    #[serde(default)]
    pub engine: SubconsciousEngine,
}

#[cfg(test)]
#[path = "subconscious_tests.rs"]
mod tests;
