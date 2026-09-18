//! Migration 11 → 12: retire the removed Medulla subconscious engine.
//!
//! Older configurations may select `engine = "medulla"`. The compatibility
//! enum variant keeps those files deserializable, while this migration makes
//! the persisted selection explicit and safe for the local engine now that
//! the Medulla implementation has been removed.

use crate::config::{schema::SubconsciousEngine, Config};

/// Rewrite a legacy Medulla selection to the supported local engine.
///
/// The `medulla_local` section is intentionally left untouched so legacy
/// configuration remains lossless if it is inspected or rewritten by a
/// later tool. The caller persists the mutation and advances the schema
/// version.
pub fn run(config: &mut Config) -> anyhow::Result<bool> {
    if config.subconscious.engine == SubconsciousEngine::Medulla {
        config.subconscious.engine = SubconsciousEngine::Local;
        log::info!(
            "[migrations][retire-subconscious-medulla] engine=medulla -> engine=local"
        );
        Ok(true)
    } else {
        log::debug!(
            "[migrations][retire-subconscious-medulla] no legacy Medulla engine selection found"
        );
        Ok(false)
    }
}

#[cfg(test)]
#[path = "retire_subconscious_medulla_tests.rs"]
mod tests;
