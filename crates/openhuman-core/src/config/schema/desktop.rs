//! Desktop automation confirmation policy.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DesktopConfig {
    /// When false, tinydesktop's one-use confirmation stop is continued by
    /// the core after the module reobserves and validates the target. This is
    /// the current product default. True enables the trusted Connections
    /// approval card and explicit continuation RPC.
    pub approvals_enabled: bool,
}

#[cfg(test)]
#[path = "desktop_tests.rs"]
mod tests;
