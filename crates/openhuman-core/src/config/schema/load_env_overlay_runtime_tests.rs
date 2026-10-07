use super::*;

#[test]
fn env_overlay_subsystems_memory_driver_and_hooks_apply() {
    let mut cfg = Config::default();
    // The shared schema retains the persisted legacy id; binding normalizes it
    // to the built-in `tinymemory` module id.
    assert_eq!(cfg.subsystems.memory.driver, "tinycortex");
    assert!(cfg.subsystems.memory.hooks.auto_recall);
    assert!(cfg.subsystems.memory.hooks.auto_capture);
    assert_eq!(cfg.subsystems.memory.hooks.max_context_tokens, 2000);
    assert_eq!(cfg.subsystems.memory.hooks.recall_max_chars, 1000);
    assert_eq!(cfg.subsystems.memory.hooks.capture_max_chars, 500);

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_MEMORY_DRIVER", "supermemory")
            .with("OPENHUMAN_MEMORY_HOOKS_AUTO_RECALL", "off")
            .with("OPENHUMAN_MEMORY_HOOKS_AUTO_CAPTURE", "false")
            .with("OPENHUMAN_MEMORY_HOOKS_MAX_CONTEXT_TOKENS", "4000")
            .with("OPENHUMAN_MEMORY_HOOKS_RECALL_MAX_CHARS", "2000")
            .with("OPENHUMAN_MEMORY_HOOKS_CAPTURE_MAX_CHARS", "900"),
    );

    assert_eq!(cfg.subsystems.memory.driver, "supermemory");
    assert!(!cfg.subsystems.memory.hooks.auto_recall);
    assert!(!cfg.subsystems.memory.hooks.auto_capture);
    assert_eq!(cfg.subsystems.memory.hooks.max_context_tokens, 4000);
    assert_eq!(cfg.subsystems.memory.hooks.recall_max_chars, 2000);
    assert_eq!(cfg.subsystems.memory.hooks.capture_max_chars, 900);

    // A blank driver value is ignored, leaving the previous override intact.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_MEMORY_DRIVER", "  "));
    assert_eq!(cfg.subsystems.memory.driver, "supermemory");

    // A non-numeric budget value is ignored, leaving the previous value intact.
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_MEMORY_HOOKS_MAX_CONTEXT_TOKENS", "nope"),
    );
    assert_eq!(cfg.subsystems.memory.hooks.max_context_tokens, 4000);
}

#[test]
fn env_overlay_learning_source_values_and_invalid_ignored() {
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_LEARNING_REFLECTION_SOURCE", "local"),
    );
    assert_eq!(
        cfg.learning.reflection_source,
        crate::config::ReflectionSource::Local
    );

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_LEARNING_REFLECTION_SOURCE", "cloud"),
    );
    assert_eq!(
        cfg.learning.reflection_source,
        crate::config::ReflectionSource::Cloud
    );

    // Unknown — ignored, retains cloud from previous step.
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_LEARNING_REFLECTION_SOURCE", "bogus"),
    );
    assert_eq!(
        cfg.learning.reflection_source,
        crate::config::ReflectionSource::Cloud
    );
}

#[test]
fn env_overlay_learning_numeric_values_parse() {
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_LEARNING_MAX_REFLECTIONS_PER_SESSION", "8")
            .with("OPENHUMAN_LEARNING_MIN_TURN_COMPLEXITY", "2"),
    );
    assert_eq!(cfg.learning.max_reflections_per_session, 8);
    assert_eq!(cfg.learning.min_turn_complexity, 2);
}
