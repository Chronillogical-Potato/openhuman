use super::*;
#[test]
fn env_overlay_sentry_dsn_trims_and_ignores_blank() {
    let mut cfg = Config::default();
    cfg.observability.sentry_dsn = None;

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_SENTRY_DSN", "  https://t@sentry.io/42  "),
    );
    assert_eq!(
        cfg.observability.sentry_dsn.as_deref(),
        Some("https://t@sentry.io/42")
    );

    // Blank value — ignored (previous DSN retained).
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_SENTRY_DSN", "   "));
    assert_eq!(
        cfg.observability.sentry_dsn.as_deref(),
        Some("https://t@sentry.io/42")
    );
}

#[test]
fn env_overlay_prefers_namespaced_core_sentry_dsn() {
    let mut cfg = Config::default();
    cfg.observability.sentry_dsn = None;

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_SENTRY_DSN", "https://legacy@sentry.io/1")
            .with("OPENHUMAN_CORE_SENTRY_DSN", "https://new@sentry.io/2"),
    );
    assert_eq!(
        cfg.observability.sentry_dsn.as_deref(),
        Some("https://new@sentry.io/2"),
        "OPENHUMAN_CORE_SENTRY_DSN must win over OPENHUMAN_SENTRY_DSN"
    );
}

#[test]
fn env_overlay_namespaced_core_sentry_dsn_works_alone() {
    let mut cfg = Config::default();
    cfg.observability.sentry_dsn = None;

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_CORE_SENTRY_DSN", "https://token@sentry.io/3"),
    );
    assert_eq!(
        cfg.observability.sentry_dsn.as_deref(),
        Some("https://token@sentry.io/3")
    );
}

#[test]
fn env_overlay_analytics_enabled_parses_truthy_falsy() {
    let mut cfg = Config::default();
    cfg.observability.analytics_enabled = false;
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_ANALYTICS_ENABLED", "1"));
    assert!(cfg.observability.analytics_enabled);

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_ANALYTICS_ENABLED", "0"));
    assert!(!cfg.observability.analytics_enabled);
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

#[test]
fn env_overlay_dictation_activation_mode_only_toggle_or_push() {
    let mut cfg = Config::default();

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_DICTATION_ACTIVATION_MODE", "toggle"),
    );
    assert_eq!(
        cfg.dictation.activation_mode,
        crate::config::DictationActivationMode::Toggle
    );

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_DICTATION_ACTIVATION_MODE", "push"),
    );
    assert_eq!(
        cfg.dictation.activation_mode,
        crate::config::DictationActivationMode::Push
    );

    // Unknown — retains previous value (Push).
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_DICTATION_ACTIVATION_MODE", "wave"),
    );
    assert_eq!(
        cfg.dictation.activation_mode,
        crate::config::DictationActivationMode::Push
    );
}

#[test]
fn env_overlay_context_tool_result_budget_env_suppresses_legacy_migration() {
    // If the env var is *present*, the `agent.tool_result_budget_bytes`
    // migration must NOT run — even when the explicit env value equals
    // the default. This protects users who explicitly set the env to
    // the default.
    let default_budget = crate::agent::context::DEFAULT_TOOL_RESULT_BUDGET_BYTES;
    let mut cfg = Config::default();
    cfg.context.tool_result_budget_bytes = default_budget;
    cfg.agent.tool_result_budget_bytes = 999_999;

    cfg.apply_env_overlay_with(&HashMapEnv::new().with(
        "OPENHUMAN_CONTEXT_TOOL_RESULT_BUDGET_BYTES",
        &default_budget.to_string(),
    ));
    assert_eq!(
        cfg.context.tool_result_budget_bytes, default_budget,
        "env presence must suppress the legacy agent→context copy"
    );
}

#[test]
fn env_overlay_compaction_default_on_and_switch() {
    // Default is on: large results become a stats line, a head and a handle
    // the REPL tools query.
    assert!(Config::default().context.compaction_enabled);
    assert!(Config::default().tokenjuice.router_enabled);
    assert!(Config::default().tokenjuice.repl_handle_enabled);

    // `OPENHUMAN_COMPACTION=0` opts out; `=1` keeps it on.
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_COMPACTION", "0"));
    assert!(!cfg.context.compaction_enabled);
    assert!(!crate::inference::tokenjuice::repl_handle_active(&cfg));
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_COMPACTION", "1"));
    assert!(cfg.context.compaction_enabled);

    // Truthy re-enables; the namespaced alias works too.
    let mut cfg = Config::default();
    cfg.context.compaction_enabled = false;
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_CONTEXT_COMPACTION_ENABLED", "on"),
    );
    assert!(cfg.context.compaction_enabled);

    // Garbage is ignored (leaves the prior value untouched).
    let mut cfg = Config::default();
    cfg.context.compaction_enabled = true;
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_COMPACTION", "maybe"));
    assert!(cfg.context.compaction_enabled);
}

#[test]
fn env_overlay_context_tool_result_budget_legacy_migration_when_env_absent() {
    // Env absent, context at default, agent customised → agent value copies forward.
    let default_budget = crate::agent::context::DEFAULT_TOOL_RESULT_BUDGET_BYTES;
    let mut cfg = Config::default();
    cfg.context.tool_result_budget_bytes = default_budget;
    cfg.agent.tool_result_budget_bytes = 777_777;

    cfg.apply_env_overlay_with(&HashMapEnv::new());
    assert_eq!(cfg.context.tool_result_budget_bytes, 777_777);
}

#[test]
fn env_overlay_context_tool_result_budget_env_wins_over_legacy_migration() {
    // Env present with a non-default value, and agent also customised.
    // The env value must apply; the legacy agent→context copy must NOT
    // overwrite it.
    let mut cfg = Config::default();
    cfg.agent.tool_result_budget_bytes = 111_111;

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_CONTEXT_TOOL_RESULT_BUDGET_BYTES", "222222"),
    );
    assert_eq!(
        cfg.context.tool_result_budget_bytes, 222_222,
        "env value wins; legacy migration suppressed"
    );
}

#[test]
fn env_overlay_auto_update_interval_parses_u32() {
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_AUTO_UPDATE_ENABLED", "true")
            .with("OPENHUMAN_AUTO_UPDATE_INTERVAL_MINUTES", "60"),
    );
    assert!(cfg.update.enabled);
    assert_eq!(cfg.update.interval_minutes, 60);

    // Garbage numeric — ignored, previous value retained.
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_AUTO_UPDATE_INTERVAL_MINUTES", "hello"),
    );
    assert_eq!(cfg.update.interval_minutes, 60);
}

#[test]
fn env_overlay_auto_update_restart_strategy_accepts_supported_values() {
    let mut cfg = Config::default();
    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_AUTO_UPDATE_RESTART_STRATEGY", "supervisor"),
    );
    assert_eq!(
        cfg.update.restart_strategy,
        crate::config::UpdateRestartStrategy::Supervisor
    );

    cfg.apply_env_overlay_with(
        &HashMapEnv::new().with("OPENHUMAN_AUTO_UPDATE_RESTART_STRATEGY", "self_replace"),
    );
    assert_eq!(
        cfg.update.restart_strategy,
        crate::config::UpdateRestartStrategy::SelfReplace
    );
}

#[test]
fn env_overlay_tool_dispatcher_overrides_the_agent_field_when_non_blank() {
    let mut cfg = Config::default();
    assert_eq!(cfg.agent.tool_dispatcher, "auto");

    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TOOL_DISPATCHER", " native "));
    assert_eq!(cfg.agent.tool_dispatcher, "native");

    // Blank values leave the persisted choice alone.
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TOOL_DISPATCHER", "   "));
    assert_eq!(cfg.agent.tool_dispatcher, "native");
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_TOOL_DISPATCHER", ""));
    assert_eq!(cfg.agent.tool_dispatcher, "native");
}

#[test]
fn env_overlay_composio_mode_overrides_when_non_blank() {
    let mut cfg = Config::default();
    assert_eq!(cfg.composio.mode, "backend");
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_COMPOSIO_MODE", " Disabled "));
    assert_eq!(cfg.composio.mode, "disabled");
    cfg.apply_env_overlay_with(&HashMapEnv::new().with("OPENHUMAN_COMPOSIO_MODE", "  "));
    assert_eq!(cfg.composio.mode, "disabled");
}

#[test]
fn env_overlay_jev_route_and_base_url_override_tool_search_when_non_blank() {
    let mut cfg = Config::default();
    assert_eq!(cfg.agent.tool_search.jev_route, "auto");
    assert_eq!(cfg.agent.tool_search.jev_base_url, None);

    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_JEV_ROUTE", " OpenRouter ")
            .with("OPENHUMAN_JEV_BASE_URL", " http://127.0.0.1:18080 "),
    );
    assert_eq!(cfg.agent.tool_search.jev_route, "openrouter");
    assert_eq!(
        cfg.agent.tool_search.jev_base_url.as_deref(),
        Some("http://127.0.0.1:18080")
    );

    // Blank values leave the persisted choice alone.
    cfg.apply_env_overlay_with(
        &HashMapEnv::new()
            .with("OPENHUMAN_JEV_ROUTE", "  ")
            .with("OPENHUMAN_JEV_BASE_URL", ""),
    );
    assert_eq!(cfg.agent.tool_search.jev_route, "openrouter");
    assert_eq!(
        cfg.agent.tool_search.jev_base_url.as_deref(),
        Some("http://127.0.0.1:18080")
    );
}

/// Local model tier presets were removed: OpenHuman no longer picks models by
/// RAM tier. A stale `OPENHUMAN_LOCAL_AI_TIER` in the environment must be
/// ignored rather than rewriting the user's configured local models.
#[test]
fn env_overlay_ignores_removed_local_ai_tier_var() {
    let env = HashMapEnv::new().with("OPENHUMAN_LOCAL_AI_TIER", "ram_2_4gb");
    let mut cfg = Config::default();
    cfg.local_ai.chat_model_id = "llama3.1:8b".to_string();
    cfg.local_ai.embedding_model_id = "nomic-embed-text:latest".to_string();
    cfg.apply_env_overlay_with(&env);
    assert_eq!(cfg.local_ai.chat_model_id, "llama3.1:8b");
    assert_eq!(cfg.local_ai.embedding_model_id, "nomic-embed-text:latest");
}

/// A config.toml written while OpenHuman still downloaded local models carries
/// tier, quantization, preload, binary-path and download-URL keys under
/// `[local_ai]`. Those keys are no longer read, but such a file must still
/// load with the user's endpoint and model choices intact.
#[test]
fn legacy_local_ai_download_keys_still_load() {
    let legacy = r#"
api_url = "http://127.0.0.1:9"

[local_ai]
runtime_enabled = true
opt_in_confirmed = true
provider = "ollama"
base_url = "http://127.0.0.1:11434"
chat_model_id = "llama3.1:8b"
embedding_model_id = "bge-m3"
selected_tier = "ram_2_4gb"
quantization = "q4_k_m"
preload_vision_model = true
preload_embedding_model = true
preload_stt_model = false
preload_tts_voice = false
ollama_binary_path = "/opt/openhuman/bin/ollama"
download_url = "https://example.invalid/model.gguf"
stt_download_url = "https://example.invalid/stt.bin"
tts_download_url = "https://example.invalid/voice.onnx"
tts_config_download_url = "https://example.invalid/voice.onnx.json"
"#;
    let cfg: Config = toml::from_str(legacy).expect("legacy local_ai keys must still parse");
    assert!(cfg.local_ai.runtime_enabled);
    assert!(cfg.local_ai.opt_in_confirmed);
    assert_eq!(cfg.local_ai.provider, "ollama");
    assert_eq!(
        cfg.local_ai.base_url.as_deref(),
        Some("http://127.0.0.1:11434")
    );
    assert_eq!(cfg.local_ai.chat_model_id, "llama3.1:8b");
    assert_eq!(cfg.local_ai.embedding_model_id, "bge-m3");

    // The runtime projection carries only endpoint and model settings.
    let runtime = crate::inference::local_runtime_config(&cfg);
    assert_eq!(runtime.local_ai.chat_model_id, "llama3.1:8b");
    assert_eq!(
        runtime.local_ai.base_url.as_deref(),
        Some("http://127.0.0.1:11434")
    );
}
