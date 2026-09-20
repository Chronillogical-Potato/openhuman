use super::*;
use crate::config::{Config, MODEL_MANAGED_DEFAULT};

#[test]
fn rewrites_stale_reasoning_v1_to_the_managed_default() {
    // `reasoning-v1` is a stale former DEFAULT_MODEL — the substantive change.
    let mut config = Config::default();
    config.default_model = Some("reasoning-v1".to_string());

    let stats = run(&mut config).expect("migration should succeed");

    assert!(stats.default_model_normalized);
    assert_eq!(config.default_model.as_deref(), Some(MODEL_MANAGED_DEFAULT));
}

#[test]
fn rewrites_padded_reasoning_v1_to_chat_v1() {
    let mut config = Config::default();
    config.default_model = Some("  reasoning-v1  ".to_string());

    let stats = run(&mut config).expect("migration should succeed");

    assert!(stats.default_model_normalized);
    assert_eq!(config.default_model.as_deref(), Some(MODEL_MANAGED_DEFAULT));
}

#[test]
fn rewrites_deprecated_reasoning_quick_v1_alias_to_chat_v1() {
    // `reasoning-quick-v1` was the chat alias; it lands on the managed default too.
    let mut config = Config::default();
    config.default_model = Some("reasoning-quick-v1".to_string());

    let stats = run(&mut config).expect("migration should succeed");

    assert!(stats.default_model_normalized);
    assert_eq!(config.default_model.as_deref(), Some(MODEL_MANAGED_DEFAULT));
}

#[test]
fn leaves_chat_v1_unchanged() {
    let mut config = Config::default();
    config.default_model = Some(MODEL_MANAGED_DEFAULT.to_string());

    let stats = run(&mut config).expect("migration should succeed");

    assert!(!stats.default_model_normalized);
    assert_eq!(config.default_model.as_deref(), Some(MODEL_MANAGED_DEFAULT));
}

#[test]
fn leaves_arbitrary_custom_value_unchanged() {
    // `default_model` round-trips custom/BYOK ids; the migration must not
    // clobber them (config-mutation contract; config_*_e2e round-trip tests).
    let mut config = Config::default();
    config.default_model = Some("worker-a-updated".to_string());

    let stats = run(&mut config).expect("migration should succeed");

    assert!(!stats.default_model_normalized);
    assert_eq!(config.default_model.as_deref(), Some("worker-a-updated"));
}

#[test]
fn leaves_other_known_tier_unchanged() {
    // An explicit non-reasoning tier (e.g. agentic) is a deliberate value.
    let mut config = Config::default();
    config.default_model = Some("agentic-v1".to_string());

    let stats = run(&mut config).expect("migration should succeed");

    assert!(!stats.default_model_normalized);
    assert_eq!(config.default_model.as_deref(), Some("agentic-v1"));
}

#[test]
fn leaves_none_unchanged() {
    let mut config = Config::default();
    config.default_model = None;

    let stats = run(&mut config).expect("migration should succeed");

    assert!(!stats.default_model_normalized);
    assert_eq!(config.default_model, None);
}
