//! `[web_chat]` — behaviour of the web chat channel's presentation layer
//! (`crate::web_chat::presentation`) that is not itself business logic, just
//! a user-facing on/off switch.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::defaults;

/// Settings for the web chat surface's post-turn presentation features.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct WebChatConfig {
    /// Whether `deliver_response` spawns the cheap follow-up-suggestions
    /// model call after `chat_done` (`web_chat::suggestions`). Defaults to
    /// `true`; set `false` to skip the extra local/summarization-role model
    /// call entirely (e.g. a constrained or offline install).
    #[serde(default = "default_true")]
    pub suggestions_enabled: bool,
}

impl Default for WebChatConfig {
    fn default() -> Self {
        Self {
            suggestions_enabled: true,
        }
    }
}

fn default_true() -> bool {
    defaults::default_true()
}

#[cfg(test)]
#[path = "web_chat_config_tests.rs"]
mod tests;
