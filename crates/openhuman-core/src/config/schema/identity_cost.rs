//! Cost tracking configuration.
//!
//! Identity is loaded from OpenClaw markdown files in the workspace
//! (`IDENTITY.md`, `SOUL.md`, etc.) and needs no config surface.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CostConfig {
    /// Retained for **recording**, not enforcement: nothing refuses a request
    /// on cost any more. `CostTracker::record_usage` is a no-op when this is
    /// `false`; `record_usage_unconditional` (the dashboard/telemetry path)
    /// ignores it.
    ///
    /// Dashboard telemetry uses `record_usage_unconditional`, so this flag
    /// does not disable telemetry capture. The dashboard
    /// JSONL store at `{workspace}/state/costs.jsonl` is populated by
    /// [`crate::platform::cost::record_provider_usage`] regardless of
    /// this flag, so users can review historical usage. Set
    /// `dashboard.enabled = false` to hide the
    /// Settings panel; delete the JSONL file to clear collected
    /// history. The file is local and never leaves the workspace.
    #[serde(default = "default_cost_enabled")]
    pub enabled: bool,

    /// Legacy monthly display target in USD (default: 100.00).
    ///
    /// **This is a display target, not a cap.** Nothing in the core refuses a
    /// request when it is exceeded — the enforcement path was removed with the
    /// spend cap. It remains in the dashboard RPC payload for compatibility,
    /// but the UI no longer presents it as a limit.
    ///
    /// Counts **managed (OpenHuman-credit) spend only** — see
    /// [`crate::platform::cost::route`]. Bring-your-own-key and local
    /// inference is billed by the user's own provider, so it is recorded for
    /// the dashboard but never counted here (#5016); driving the gauge off the
    /// all-route total filled a pure-BYOK user's bar against a limit that
    /// could never fire.
    ///
    /// A retired `daily_limit_usd` key may still be present in existing config
    /// files. It is accepted and ignored — this struct does not
    /// `deny_unknown_fields`, so upgrading never fails to parse.
    #[serde(default = "default_monthly_limit")]
    pub monthly_limit_usd: f64,

    /// Per-model pricing (USD per 1M tokens)
    #[serde(default)]
    pub prices: HashMap<String, ModelPricing>,

    /// Dashboard chart panel configuration. Drives the 7-day cost / token
    /// visualisation in Settings → Cost dashboard.
    #[serde(default)]
    pub dashboard: CostDashboardConfig,
}

/// Configuration for the 7-day cost & token usage dashboard panel.
///
/// Legacy thresholds are retained in the dashboard RPC payload for
/// compatibility; the UI does not display budget warnings.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CostDashboardConfig {
    /// Whether the dashboard panel is enabled in the UI. The panel still
    /// renders a disabled hint when this is false.
    #[serde(default = "default_dashboard_enabled")]
    pub enabled: bool,

    /// Display currency label. Amounts are always stored in USD; this is
    /// purely a presentation hint.
    #[serde(default = "default_currency")]
    pub currency: String,

    /// Warn threshold as a fraction of the monthly budget (default: 0.8).
    /// Bars and status flip to amber once month-to-date utilisation reaches
    /// this value.
    #[serde(default = "default_warn_threshold")]
    pub warn_threshold: f64,

    /// Alert threshold as a fraction of the monthly budget (default: 0.95).
    /// Bars and status flip to red once month-to-date utilisation reaches
    /// this value.
    #[serde(default = "default_alert_threshold")]
    pub alert_threshold: f64,
}

impl Default for CostDashboardConfig {
    fn default() -> Self {
        Self {
            enabled: default_dashboard_enabled(),
            currency: default_currency(),
            warn_threshold: default_warn_threshold(),
            alert_threshold: default_alert_threshold(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ModelPricing {
    /// Input price per 1M tokens
    #[serde(default)]
    pub input: f64,

    /// Output price per 1M tokens
    #[serde(default)]
    pub output: f64,
}

fn default_cost_enabled() -> bool {
    true
}

fn default_monthly_limit() -> f64 {
    100.0
}

fn default_dashboard_enabled() -> bool {
    true
}

fn default_currency() -> String {
    "USD".to_string()
}

fn default_warn_threshold() -> f64 {
    0.8
}

fn default_alert_threshold() -> f64 {
    0.95
}

impl Default for CostConfig {
    fn default() -> Self {
        Self {
            enabled: default_cost_enabled(),
            monthly_limit_usd: default_monthly_limit(),
            prices: get_default_pricing(),
            dashboard: CostDashboardConfig::default(),
        }
    }
}

/// Default pricing for the managed default model (USD per 1M tokens).
///
/// DeepSeek V4 Flash through the managed OpenRouter passthrough. Other catalog
/// models the user pins are priced from the catalog the backend serves
/// (`inference_list_models`), not from here.
fn get_default_pricing() -> HashMap<String, ModelPricing> {
    use super::types::MODEL_MANAGED_DEFAULT;

    let mut prices = HashMap::new();
    prices.insert(
        MODEL_MANAGED_DEFAULT.into(),
        ModelPricing {
            input: 0.0886,
            output: 0.1772,
        },
    );
    prices
}

#[cfg(test)]
#[path = "identity_cost_tests.rs"]
mod tests;
