//! OpenHuman configuration mapping for TinyInference local-model presets.

use crate::config::schema::LocalAiConfig;
use tinyinference_local::presets::{all_presets, preset_for_tier, ModelTier, VisionMode};

pub fn vision_mode_for_config(config: &LocalAiConfig) -> VisionMode {
    match current_tier_from_config(config) {
        ModelTier::Custom => {
            if config.vision_model_id.trim().is_empty() {
                VisionMode::Disabled
            } else if config.preload_vision_model {
                VisionMode::Bundled
            } else {
                VisionMode::Ondemand
            }
        }
        tier => tinyinference_local::presets::vision_mode_for_tier(tier),
    }
}

pub fn supports_screen_summary(config: &LocalAiConfig) -> bool {
    !matches!(vision_mode_for_config(config), VisionMode::Disabled)
}

pub fn apply_preset_to_config(config: &mut LocalAiConfig, tier: ModelTier) {
    if let Some(preset) = preset_for_tier(tier) {
        tracing::debug!(
            ?tier,
            chat = preset.chat_model_id,
            vision_mode = ?preset.vision_mode,
            "[local_ai] applying preset to config"
        );
        config.model_id = preset.chat_model_id.to_string();
        config.chat_model_id = preset.chat_model_id.to_string();
        config.vision_model_id = preset.vision_model_id.to_string();
        config.embedding_model_id = preset.embedding_model_id.to_string();
        config.quantization = preset.quantization.to_string();
        config.preload_vision_model = matches!(preset.vision_mode, VisionMode::Bundled);
        config.preload_embedding_model = true;
        config.selected_tier = Some(tier.as_str().to_string());
        config.runtime_enabled = true;
    } else {
        tracing::debug!("[local_ai] apply_preset_to_config called for custom tier; no-op");
    }
}

pub fn current_tier_from_config(config: &LocalAiConfig) -> ModelTier {
    if let Some(ref stored) = config.selected_tier {
        if let Some(tier) = ModelTier::from_str_opt(stored) {
            if tier == ModelTier::Custom {
                return ModelTier::Custom;
            }
            if let Some(preset) = preset_for_tier(tier) {
                if preset_matches_config(&preset, config) {
                    return tier;
                }
            }
        }
    }

    all_presets()
        .into_iter()
        .find(|preset| preset_matches_config(preset, config))
        .map_or(ModelTier::Custom, |preset| preset.tier)
}

fn preset_matches_config(
    preset: &tinyinference_local::presets::ModelPreset,
    config: &LocalAiConfig,
) -> bool {
    let vision_matches = if matches!(preset.vision_mode, VisionMode::Disabled) {
        config.vision_model_id.trim().is_empty()
    } else {
        config.vision_model_id == preset.vision_model_id
    };
    config.chat_model_id == preset.chat_model_id
        && vision_matches
        && config.embedding_model_id == preset.embedding_model_id
}

#[cfg(test)]
#[path = "local_ai_presets_tests.rs"]
mod tests;
