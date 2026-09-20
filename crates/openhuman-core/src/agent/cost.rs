//! Per-turn cost accounting for an agent's tool-call loop.
//!
//! Each provider response carries an optional [`UsageInfo`] block with
//! `input_tokens`, `output_tokens`, `cached_input_tokens`, and an
//! authoritative `charged_amount_usd` populated by the OpenHuman
//! backend. [`TurnCost`] sums those across every provider call inside a
//! single turn so the harness can:
//!
//! - emit per-iteration cost telemetry via
//!   [`crate::agent::progress::AgentProgress::TurnCostUpdated`];
//! - feed budget stop hooks (mid-turn USD cap);
//! - log accurate end-of-turn cost lines.
//!
//! When `charged_amount_usd` is zero (older backend builds, providers
//! that don't surface billing), we fall back to a simple token-rate
//! estimate via [`estimate_call_cost_usd`] keyed on the model tier
//! name. The estimate is a floor — directly-billed cost from the
//! backend always wins when available.
//!
//! The pricing table is intentionally tiny: the managed default model,
//! with the retired tier slugs (`hint:chat`, …) still resolving to its rate so
//! older cost records estimate sanely. Every other model — catalog ids the
//! user pins, BYOK vendor models — is priced from the vendor catalog.

use crate::inference::provider::UsageInfo;

/// Per-million-token rates for a single model tier.
///
/// All prices are USD per million tokens. `cached_input_per_mtok_usd`
/// applies to the `cached_input_tokens` portion of the usage block (KV
/// prefix cache hits on supporting backends); the remaining
/// `input_tokens - cached_input_tokens` are charged at
/// `input_per_mtok_usd`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ModelPricing {
    /// Model id, e.g. `"openrouter/deepseek/deepseek-v4-flash"`.
    pub(crate) model: &'static str,
    /// Standard prompt rate, USD per million input tokens.
    pub(crate) input_per_mtok_usd: f64,
    /// Cached-prefix prompt rate, USD per million cached input tokens.
    pub(crate) cached_input_per_mtok_usd: f64,
    /// Completion rate, USD per million output tokens.
    pub(crate) output_per_mtok_usd: f64,
}

/// Conservative fallback when nothing in the table matches. Picked so
/// budget caps still bite on unknown models rather than reading as $0.
const FALLBACK_PRICING: ModelPricing = ModelPricing {
    model: "<fallback>",
    input_per_mtok_usd: 3.00,
    cached_input_per_mtok_usd: 0.30,
    output_per_mtok_usd: 15.00,
};

/// Static price table for the managed default model.
const PRICING_TABLE: &[ModelPricing] = &[
    // The managed default model — DeepSeek V4 Flash through the OpenRouter
    // passthrough. Estimate only; the backend's echoed `charged_amount_usd` is
    // authoritative when present. Any other catalog model the user pins is
    // priced from the vendor catalog (`platform::cost::catalog`) below.
    ModelPricing {
        model: crate::config::MODEL_MANAGED_DEFAULT,
        input_per_mtok_usd: 0.0886,
        cached_input_per_mtok_usd: 0.0886,
        output_per_mtok_usd: 0.1772,
    },
];

/// Legacy tier slugs from older transcripts and configs, mapped onto the
/// managed default's rate so an old cost record still estimates sanely.
const LEGACY_TIER_ROWS: &[&str] = &crate::config::LEGACY_TIER_MODELS;

/// Whether `model` is served by the managed OpenHuman backend: the managed
/// default, an `openrouter/...` passthrough id, a `hint:*` role alias, or a
/// retired tier slug. Anything else — BYOK vendor ids (`claude-*`, `gpt-*`) or
/// local model names — is a custom/BYO-provider model. Used by trace exporters
/// to stamp model provenance (`gen_ai.provider` = "managed" | "custom").
pub(crate) fn is_managed_tier(model: &str) -> bool {
    crate::platform::cost::route::route_for_model(model) == crate::platform::cost::route::CostRoute::Managed
        || model.trim().starts_with("hint:")
}

/// Look up pricing for a model name, falling back to [`FALLBACK_PRICING`].
///
/// Resolution order:
/// 1. Exact match on the managed default model (or a retired tier slug /
///    `hint:*` alias, which ran on it).
/// 2. The concrete-vendor-model pricing catalog
///    ([`crate::platform::cost::catalog`]) — accurate per-model rates for
///    `claude-*`, `gpt-*`, `gemini-*`, `deepseek-*`, `kimi-*`, `qwen-*`,
///    `mistral-*`, including OpenRouter-style `vendor/model` ids.
/// 3. [`FALLBACK_PRICING`].
pub(crate) fn lookup_pricing(model: &str) -> ModelPricing {
    let trimmed = model.trim();
    if let Some(row) = PRICING_TABLE.iter().find(|row| row.model == trimmed) {
        return *row;
    }
    if trimmed.starts_with("hint:") || LEGACY_TIER_ROWS.contains(&trimmed) {
        return PRICING_TABLE[0];
    }
    if let Some(price) = crate::platform::cost::catalog::lookup(model) {
        return ModelPricing {
            model: price.model_id,
            input_per_mtok_usd: price.input_per_mtok_usd,
            cached_input_per_mtok_usd: price.cached_input_per_mtok_usd,
            output_per_mtok_usd: price.output_per_mtok_usd,
        };
    }
    FALLBACK_PRICING
}

/// Estimate the USD cost of a single provider call from its token
/// usage. Used as a fallback when `charged_amount_usd` is missing.
pub fn estimate_call_cost_usd(model: &str, usage: &UsageInfo) -> f64 {
    let pricing = lookup_pricing(model);
    let cached = usage.cached_input_tokens;
    let standard_input = usage.input_tokens.saturating_sub(cached);
    let m = 1_000_000.0_f64;
    (standard_input as f64) / m * pricing.input_per_mtok_usd
        + (cached as f64) / m * pricing.cached_input_per_mtok_usd
        + (usage.output_tokens as f64) / m * pricing.output_per_mtok_usd
}

/// Pick the most authoritative USD figure for a single provider call.
///
/// Backend-reported `charged_amount_usd` wins whenever it's > 0;
/// otherwise we fall back to [`estimate_call_cost_usd`].
pub fn call_cost_usd(model: &str, usage: &UsageInfo) -> f64 {
    if usage.charged_amount_usd > 0.0 {
        usage.charged_amount_usd
    } else {
        estimate_call_cost_usd(model, usage)
    }
}

/// Running cost / token tally across every provider call inside a
/// single turn of the tool-call loop.
///
/// `charged_usd` is the sum of authoritative `charged_amount_usd`
/// values; `estimated_usd` adds the fallback estimate for any call that
/// lacked one. `total_usd()` returns whichever has more signal.
#[derive(Debug, Clone, Default)]
pub struct TurnCost {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub charged_usd: f64,
    pub estimated_usd: f64,
    pub call_count: u32,
}

impl TurnCost {
    /// New empty accumulator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold a single provider call's usage into the running totals.
    pub fn add_call(&mut self, model: &str, usage: &UsageInfo) {
        self.input_tokens = self.input_tokens.saturating_add(usage.input_tokens);
        self.output_tokens = self.output_tokens.saturating_add(usage.output_tokens);
        self.cached_input_tokens = self
            .cached_input_tokens
            .saturating_add(usage.cached_input_tokens);
        if usage.charged_amount_usd > 0.0 {
            self.charged_usd += usage.charged_amount_usd;
        } else {
            self.estimated_usd += estimate_call_cost_usd(model, usage);
        }
        self.call_count = self.call_count.saturating_add(1);
    }

    /// Best-available USD figure: authoritative charged amount plus
    /// estimated cost for any calls that didn't carry one.
    pub fn total_usd(&self) -> f64 {
        self.charged_usd + self.estimated_usd
    }
}

#[cfg(test)]
#[path = "cost_tests.rs"]
mod tests;
