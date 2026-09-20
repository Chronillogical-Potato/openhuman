//! Migration 12 → 13: retire the managed tier slugs (`chat-v1`, `agentic-v1`,
//! `reasoning-v1`, …) in favour of concrete OpenRouter model ids.
//!
//! The managed backend no longer serves tier endpoints; every managed workload
//! runs on the pinned default model or [`MODEL_MANAGED_DEFAULT`]. A persisted
//! tier slug would reach the backend verbatim and 400, so every place a user
//! (or an older build) could have written one is rewritten:
//!
//! - `default_model` — any tier slug becomes [`MODEL_MANAGED_DEFAULT`].
//! - `orchestrator.model`, every `teams.<name>.{lead,agent}_model` and each
//!   `agents.<id>.model` delegate pin — tier slugs become the managed default
//!   too; a slug that was pinning a *role* (`reasoning-v1` on a lead)
//!   loses nothing, since on the managed backend every role now runs the same
//!   model.
//! - `model_routes[].model` — the same rewrite; the `hint` key is untouched.
//!
//! Concrete ids (`openrouter/...`, BYOK model names) and `hint:*` markers are
//! left exactly as they are: `hint:*` is still translated at dispatch, and a
//! concrete id is the user's own choice.
//!
//! Pure in-memory mutation; the caller persists and bumps `schema_version`.
//! Idempotent: a config with no tier slug is a no-op.

use crate::config::{is_legacy_tier_model, Config, MODEL_MANAGED_DEFAULT};

/// Counters returned by [`run`] for diagnostics.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MigrationStats {
    /// Number of fields rewritten from a tier slug to the managed default.
    pub rewritten: usize,
}

fn rewrite(field: &str, value: &mut String, stats: &mut MigrationStats) {
    if is_legacy_tier_model(value) {
        log::info!(
            "[migrations][retire-managed-tiers] {field}: {:?} -> {MODEL_MANAGED_DEFAULT:?}",
            value
        );
        *value = MODEL_MANAGED_DEFAULT.to_string();
        stats.rewritten += 1;
    }
}

fn rewrite_opt(field: &str, value: &mut Option<String>, stats: &mut MigrationStats) {
    if let Some(inner) = value.as_mut() {
        rewrite(field, inner, stats);
    }
}

/// Rewrite every retired tier slug in `config` to the managed default model.
pub fn run(config: &mut Config) -> anyhow::Result<MigrationStats> {
    let mut stats = MigrationStats::default();

    rewrite_opt("default_model", &mut config.default_model, &mut stats);
    rewrite_opt(
        "orchestrator.model",
        &mut config.orchestrator.model,
        &mut stats,
    );
    for (name, team) in config.teams.iter_mut() {
        rewrite_opt(
            &format!("teams.{name}.lead_model"),
            &mut team.lead_model,
            &mut stats,
        );
        rewrite_opt(
            &format!("teams.{name}.agent_model"),
            &mut team.agent_model,
            &mut stats,
        );
    }
    for (id, delegate) in config.agents.iter_mut() {
        rewrite(&format!("agents.{id}.model"), &mut delegate.model, &mut stats);
    }
    for route in config.model_routes.iter_mut() {
        rewrite(
            &format!("model_routes[{}].model", route.hint),
            &mut route.model,
            &mut stats,
        );
    }

    log::debug!(
        "[migrations][retire-managed-tiers] rewritten={} default_model={:?}",
        stats.rewritten,
        config.default_model
    );
    Ok(stats)
}

#[cfg(test)]
#[path = "retire_managed_tier_slugs_tests.rs"]
mod tests;
