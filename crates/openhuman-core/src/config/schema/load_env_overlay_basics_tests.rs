use super::*;

#[test]
fn env_overlay_model_only_honours_namespaced_var() {
    // Both set → OPENHUMAN_MODEL wins; bare MODEL is ignored even when
    // OPENHUMAN_MODEL is absent.
    let env = HashMapEnv::new()
        .with("OPENHUMAN_MODEL", "specific-v2")
        .with("MODEL", "alias-fallback");
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(&env);
    assert_eq!(cfg.default_model.as_deref(), Some("specific-v2"));

    // Only bare MODEL set → must NOT clobber default_model. Vendor
    // asset-tag env vars (e.g. Dell OptiPlex `MODEL=7080`) would otherwise
    // hijack the LLM model name and 400 every backend call
    // (Sentry OPENHUMAN-TAURI-J8).
    let env = HashMapEnv::new().with("MODEL", "7080");
    let mut cfg = Config::default();
    let original = cfg.default_model.clone();
    cfg.apply_env_overlay_with(&env);
    assert_eq!(
        cfg.default_model, original,
        "bare MODEL env var must not override default_model"
    );

    // Whitespace-only OPENHUMAN_MODEL must not clobber either. Some
    // shells/CI runners pass an unset-but-declared env var through as
    // `"   "`, which `is_empty()` alone wouldn't reject.
    let env = HashMapEnv::new().with("OPENHUMAN_MODEL", "   ");
    let mut cfg = Config::default();
    let original = cfg.default_model.clone();
    cfg.apply_env_overlay_with(&env);
    assert_eq!(
        cfg.default_model, original,
        "whitespace-only OPENHUMAN_MODEL must not clobber default_model"
    );
}
#[test]
fn env_overlay_model_ignores_empty() {
    let env = HashMapEnv::new().with("OPENHUMAN_MODEL", "");
    let mut cfg = Config::default();
    let original = cfg.default_model.clone();
    cfg.apply_env_overlay_with(&env);
    assert_eq!(cfg.default_model, original, "empty value must not clobber");
}

#[test]
fn env_overlay_temperature_accepts_valid_and_ignores_out_of_range_or_garbage() {
    let mut cfg = Config::default();
    cfg.default_temperature = 0.5;

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TEMPERATURE", "1.5"));
    assert!((cfg.default_temperature - 1.5).abs() < f64::EPSILON);

    // Negative (< 0.0) — ignored.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TEMPERATURE", "-0.1"));
    assert!((cfg.default_temperature - 1.5).abs() < f64::EPSILON);

    // Above cap (> 2.0) — ignored.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TEMPERATURE", "2.5"));
    assert!((cfg.default_temperature - 1.5).abs() < f64::EPSILON);

    // Garbage — ignored.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TEMPERATURE", "nope"));
    assert!((cfg.default_temperature - 1.5).abs() < f64::EPSILON);

    // Boundaries — inclusive on both ends.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TEMPERATURE", "0"));
    assert_eq!(cfg.default_temperature, 0.0);
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TEMPERATURE", "2"));
    assert_eq!(cfg.default_temperature, 2.0);
}

#[test]
fn env_overlay_autonomy_max_actions_per_hour_accepts_valid_u32() {
    let mut cfg = Config::default();
    cfg.autonomy.max_actions_per_hour = 20;

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_MAX_ACTIONS_PER_HOUR", "64"));
    assert_eq!(cfg.autonomy.max_actions_per_hour, 64);

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_MAX_ACTIONS_PER_HOUR", "  "));
    assert_eq!(
        cfg.autonomy.max_actions_per_hour, 64,
        "blank env value must leave the configured limit unchanged"
    );

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_MAX_ACTIONS_PER_HOUR", "NaN"));
    assert_eq!(
        cfg.autonomy.max_actions_per_hour, 64,
        "invalid env value must leave the configured limit unchanged"
    );
}

#[test]
fn env_overlay_memory_sync_interval_parses_and_honours_zero() {
    let mut cfg = Config::default();
    assert!(cfg.memory_sync_interval_secs.is_none());

    // A positive value is stored verbatim.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with(MEMORY_SYNC_INTERVAL_SECS_ENV_VAR, "14400"));
    assert_eq!(cfg.memory_sync_interval_secs, Some(14_400));

    // `0` is honoured as the "Manual only" sentinel (unlike the per-provider
    // override which rejects it).
    cfg.apply_env_overlay_with(&HashMapEnv::new().with(MEMORY_SYNC_INTERVAL_SECS_ENV_VAR, "0"));
    assert_eq!(cfg.memory_sync_interval_secs, Some(0));

    // A non-numeric value is ignored, leaving the previous value intact.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with(MEMORY_SYNC_INTERVAL_SECS_ENV_VAR, "nope"));
    assert_eq!(cfg.memory_sync_interval_secs, Some(0));

    // A blank value is ignored too.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with(MEMORY_SYNC_INTERVAL_SECS_ENV_VAR, "  "));
    assert_eq!(cfg.memory_sync_interval_secs, Some(0));
}

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
fn env_overlay_output_language_accepts_non_empty_value() {
    let mut cfg = Config::default();
    assert!(cfg.output_language.is_none());

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_OUTPUT_LANGUAGE", "zh-CN"));
    assert_eq!(cfg.output_language.as_deref(), Some("zh-CN"));
    assert!(cfg
        .output_language_directive()
        .as_deref()
        .unwrap_or_default()
        .contains("Simplified Chinese"));

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_OUTPUT_LANGUAGE", "   "));
    assert_eq!(
        cfg.output_language.as_deref(),
        Some("zh-CN"),
        "blank env value must not clear an explicit config value"
    );
}

#[test]
fn env_overlay_reasoning_enabled_recognises_truthy_falsy_and_ignores_garbage() {
    let mut cfg = Config::default();
    cfg.runtime.reasoning_enabled = None;

    for truthy in ["1", "true", "yes", "on", "TRUE", " On "] {
        cfg.runtime.reasoning_enabled = None;
        cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_REASONING_ENABLED", truthy));
        assert_eq!(
            cfg.runtime.reasoning_enabled,
            Some(true),
            "truthy value {truthy:?} should enable reasoning"
        );
    }

    for falsy in ["0", "false", "no", "off", "OFF"] {
        cfg.runtime.reasoning_enabled = Some(true);
        cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_REASONING_ENABLED", falsy));
        assert_eq!(
            cfg.runtime.reasoning_enabled,
            Some(false),
            "falsy value {falsy:?} should disable reasoning"
        );
    }

    // Garbage leaves the previous value unchanged.
    cfg.runtime.reasoning_enabled = Some(true);
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_REASONING_ENABLED", "maybe"));
    assert_eq!(cfg.runtime.reasoning_enabled, Some(true));

    // Alias works when the OPENHUMAN variant is absent.
    cfg.runtime.reasoning_enabled = None;
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("REASONING_ENABLED", "yes"));
    assert_eq!(cfg.runtime.reasoning_enabled, Some(true));
}

#[test]
fn env_overlay_web_search_limits_validated() {
    let mut cfg = Config::default();
    cfg.web_search.max_results = 3;
    cfg.web_search.timeout_secs = 10;

    // Valid values apply.
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_WEB_SEARCH_MAX_RESULTS", "7")
            .with("OPENHUMAN_WEB_SEARCH_TIMEOUT_SECS", "25"),
    );
    assert_eq!(cfg.web_search.max_results, 7);
    assert_eq!(cfg.web_search.timeout_secs, 25);

    // Out-of-range — ignored.
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_WEB_SEARCH_MAX_RESULTS", "0")
            .with("OPENHUMAN_WEB_SEARCH_TIMEOUT_SECS", "0"),
    );
    assert_eq!(cfg.web_search.max_results, 7);
    assert_eq!(cfg.web_search.timeout_secs, 25);

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_WEB_SEARCH_MAX_RESULTS", "11"));
    assert_eq!(cfg.web_search.max_results, 7);

    // Bare aliases also accepted when the OPENHUMAN-prefixed variant is absent.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("WEB_SEARCH_MAX_RESULTS", "4"));
    assert_eq!(cfg.web_search.max_results, 4);
}

#[test]
fn env_overlay_tavily_api_key_supports_alias_precedence_and_ignores_blank_values() {
    let mut cfg = Config::default();

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("TAVILY_API_KEY", "alias-key"));
    assert_eq!(cfg.search.tavily.api_key.as_deref(), Some("alias-key"));

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("TAVILY_API_KEY", "lower-priority-key")
            .with("OPENHUMAN_TAVILY_API_KEY", "namespaced-key"),
    );
    assert_eq!(cfg.search.tavily.api_key.as_deref(), Some("namespaced-key"));

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TAVILY_API_KEY", "   "));
    assert_eq!(
        cfg.search.tavily.api_key.as_deref(),
        Some("namespaced-key"),
        "a blank override must not clear the configured key"
    );
}

#[test]
fn env_overlay_searxng_config_validated() {
    let mut cfg = Config::default();

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_SEARXNG_ENABLED", "true")
            .with("OPENHUMAN_SEARXNG_BASE_URL", "http://127.0.0.1:8888")
            .with("OPENHUMAN_SEARXNG_MAX_RESULTS", "40")
            .with("OPENHUMAN_SEARXNG_DEFAULT_LANGUAGE", "fr")
            .with("OPENHUMAN_SEARXNG_TIMEOUT_SECS", "9"),
    );

    assert!(cfg.searxng.enabled);
    assert_eq!(cfg.searxng.base_url, "http://127.0.0.1:8888");
    assert_eq!(cfg.searxng.max_results, 40);
    assert_eq!(cfg.searxng.default_language, "fr");
    assert_eq!(cfg.searxng.timeout_secs, 9);

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_SEARXNG_ENABLED", "no")
            .with("OPENHUMAN_SEARXNG_MAX_RESULTS", "0")
            .with("OPENHUMAN_SEARXNG_TIMEOUT_SECS", "0"),
    );

    assert!(!cfg.searxng.enabled);
    assert_eq!(cfg.searxng.max_results, 40);
    assert_eq!(cfg.searxng.timeout_secs, 9);

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("SEARXNG_TIMEOUT_SECONDS", "11"));
    assert_eq!(cfg.searxng.timeout_secs, 11);
}

#[test]
fn env_overlay_proxy_url_enables_proxy_when_not_explicit() {
    let mut cfg = Config::default();
    assert!(!cfg.proxy.enabled);

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_HTTP_PROXY", "http://proxy.local:3128"),
    );

    assert!(
        cfg.proxy.enabled,
        "setting a proxy URL without explicit enable should auto-enable"
    );
    assert_eq!(
        cfg.proxy.http_proxy.as_deref(),
        Some("http://proxy.local:3128")
    );
}

#[test]
fn env_overlay_explicit_proxy_enabled_overrides_auto_enable() {
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_PROXY_ENABLED", "false")
            .with("OPENHUMAN_HTTP_PROXY", "http://proxy.local:3128"),
    );
    assert!(
        !cfg.proxy.enabled,
        "explicit OPENHUMAN_PROXY_ENABLED=false must win over URL-driven auto-enable"
    );
}

#[test]
fn env_overlay_proxy_scope_invalid_value_leaves_scope_unchanged() {
    let mut cfg = Config::default();
    let original_scope = cfg.proxy.scope;
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_PROXY_SCOPE", "bogus-scope"));
    assert_eq!(cfg.proxy.scope, original_scope);
}

#[test]
fn env_overlay_node_flags_respect_bool_parser() {
    let mut cfg = Config::default();
    let original_version = cfg.node.version.clone();

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_NODE_ENABLED", "yes")
            .with("OPENHUMAN_NODE_PREFER_SYSTEM", "off")
            .with("OPENHUMAN_NODE_CACHE_DIR", "/tmp/oh-node"),
    );
    assert!(cfg.node.enabled);
    assert!(!cfg.node.prefer_system);
    assert_eq!(cfg.node.cache_dir, "/tmp/oh-node");
    assert_eq!(
        cfg.node.version, original_version,
        "untouched keys stay at defaults"
    );

    // Unrecognised bool — ignored, keeps previous true.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_NODE_ENABLED", "perhaps"));
    assert!(cfg.node.enabled);

    // Blank version does NOT clobber.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_NODE_VERSION", "   "));
    assert_eq!(cfg.node.version, original_version);
}

#[test]
fn env_overlay_runtime_python_flags_respect_bool_parser() {
    let mut cfg = Config::default();
    let original_version = cfg.runtime_python.minimum_version.clone();

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_RUNTIME_PYTHON_ENABLED", "yes")
            .with("OPENHUMAN_RUNTIME_PYTHON_PREFER_SYSTEM", "off")
            .with("OPENHUMAN_RUNTIME_PYTHON_CACHE_DIR", "/tmp/oh-python")
            .with("OPENHUMAN_RUNTIME_PYTHON_MANAGED_RELEASE_TAG", "20260510")
            .with("OPENHUMAN_RUNTIME_PYTHON_PREFERRED_COMMAND", "python3.12"),
    );
    assert!(cfg.runtime_python.enabled);
    assert!(!cfg.runtime_python.prefer_system);
    assert_eq!(cfg.runtime_python.cache_dir, "/tmp/oh-python");
    assert_eq!(cfg.runtime_python.managed_release_tag, "20260510");
    assert_eq!(cfg.runtime_python.preferred_command, "python3.12");
    assert_eq!(
        cfg.runtime_python.minimum_version, original_version,
        "untouched keys stay at defaults"
    );

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_RUNTIME_PYTHON_ENABLED", "perhaps"),
    );
    assert!(cfg.runtime_python.enabled);

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_RUNTIME_PYTHON_MINIMUM_VERSION", "   "),
    );
    assert_eq!(cfg.runtime_python.minimum_version, original_version);

    cfg.runtime_python.cache_dir = "/tmp/seed".into();
    cfg.runtime_python.managed_release_tag = "20260510".into();
    cfg.runtime_python.preferred_command = "python3.12".into();
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_RUNTIME_PYTHON_CACHE_DIR", "   ")
            .with("OPENHUMAN_RUNTIME_PYTHON_MANAGED_RELEASE_TAG", "   ")
            .with("OPENHUMAN_RUNTIME_PYTHON_PREFERRED_COMMAND", "   "),
    );
    assert_eq!(cfg.runtime_python.cache_dir, "");
    assert_eq!(cfg.runtime_python.managed_release_tag, "");
    assert_eq!(cfg.runtime_python.preferred_command, "");
}
