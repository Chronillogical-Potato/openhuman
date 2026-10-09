use super::{
    grouped_schemas, load_dotenv_for_cli, parse_function_params, parse_input_value,
    parse_launch_options, parse_serve_args,
};
use crate::config::test_env::EnvVarGuard;
use crate::core::{ControllerSchema, FieldSchema, TypeSchema};
use tempfile::tempdir;

#[test]
fn launch_options_parse_model_and_provider_before_command() {
    let args = vec![
        "--model".to_string(),
        "qwen3:8b".to_string(),
        "--provider-id=ollama".to_string(),
        "run".to_string(),
        "--jsonrpc-only".to_string(),
    ];
    let parsed = parse_launch_options(&args).expect("global options");
    assert_eq!(parsed.model.as_deref(), Some("qwen3:8b"));
    assert_eq!(parsed.provider.as_deref(), Some("ollama"));
    assert_eq!(parsed.args, ["run", "--jsonrpc-only"]);
}

#[test]
fn launch_options_support_short_flags_and_last_value_wins() {
    let args = ["-m", "old", "--model=new", "-p", "p_openai", "tui"].map(str::to_string);
    let parsed = parse_launch_options(&args).expect("global options");
    assert_eq!(parsed.model.as_deref(), Some("new"));
    assert_eq!(parsed.provider.as_deref(), Some("p_openai"));
    assert_eq!(parsed.args, ["tui"]);
}

#[test]
fn launch_options_stop_at_the_subcommand() {
    let args = ["inference", "list_models", "--provider", "openai"].map(str::to_string);
    let parsed = parse_launch_options(&args).expect("global options");
    assert_eq!(parsed.args, args);
    assert_eq!(parsed.provider, None);
}

#[test]
fn launch_options_reject_missing_or_empty_values() {
    for args in [
        vec!["--model".to_string()],
        vec!["--provider=".to_string()],
        vec![
            "--model".to_string(),
            "--provider".to_string(),
            "ollama".to_string(),
        ],
    ] {
        assert!(parse_launch_options(&args).is_err());
    }
}

/// Serialises env-mutating CLI tests on the crate-wide env lock and the
/// backend env lock — these tests set `BACKEND_URL`, which
/// `openhuman_tinyhumans::backend::url` tests also read/remove, and other
/// suites mutate env under `TEST_ENV_LOCK` alone, so neither lock suffices on
/// its own. Taken in the established order: `TEST_ENV_LOCK`, then backend.
fn env_lock() -> (
    tokio::sync::MutexGuard<'static, ()>,
    tokio::sync::MutexGuard<'static, ()>,
) {
    (
        crate::config::test_env::lock_env(),
        crate::config::app_env::env_test_lock(),
    )
}

#[test]
fn grouped_schemas_contains_migrated_namespaces() {
    let grouped = grouped_schemas();
    assert!(grouped.contains_key("health"));
    assert!(grouped.contains_key("doctor"));
    assert!(grouped.contains_key("encrypt"));
    assert!(grouped.contains_key("decrypt"));
    assert!(grouped.contains_key("config"));
    assert!(grouped.contains_key("auth"));
    assert!(grouped.contains_key("service"));
    assert!(grouped.contains_key("inference"));
}

#[test]
fn parse_function_params_rejects_unknown_param() {
    let schema = ControllerSchema {
        namespace: "test",
        function: "echo",
        description: "test schema",
        inputs: vec![FieldSchema {
            name: "message",
            ty: TypeSchema::String,
            required: true,
            comment: "message text",
        }],
        outputs: vec![FieldSchema {
            name: "result",
            ty: TypeSchema::String,
            required: true,
            comment: "echo response",
        }],
    };
    let args = vec!["--unknown".to_string(), "value".to_string()];
    let err = parse_function_params(&schema, &args).expect_err("unknown param should fail");
    assert!(err.contains("unknown param"));
}

#[test]
fn parse_function_params_rejects_flag_like_missing_value() {
    let schema = ControllerSchema {
        namespace: "test",
        function: "configure",
        description: "test schema",
        inputs: vec![
            FieldSchema {
                name: "enabled",
                ty: TypeSchema::Bool,
                required: true,
                comment: "whether the feature is enabled",
            },
            FieldSchema {
                name: "name",
                ty: TypeSchema::String,
                required: true,
                comment: "feature name",
            },
        ],
        outputs: vec![],
    };
    let args = vec![
        "--enabled".to_string(),
        "--name".to_string(),
        "demo".to_string(),
    ];
    let err = parse_function_params(&schema, &args).expect_err("missing value should fail");
    assert_eq!(err, "missing value for --enabled");
}

#[test]
fn parse_input_value_rejects_invalid_bool() {
    let err =
        parse_input_value(&TypeSchema::Bool, "not-a-bool").expect_err("invalid bool should fail");
    assert!(err.contains("expected bool"));
}

#[test]
fn parse_input_value_enforces_bounded_u64_range() {
    let ty = TypeSchema::BoundedU64 { min: 1, max: 10 };

    assert_eq!(parse_input_value(&ty, "10").unwrap(), serde_json::json!(10));

    let err = parse_input_value(&ty, "11").expect_err("above max should fail");
    assert_eq!(err, "expected unsigned integer <= 10, got '11'");

    let err = parse_input_value(&ty, "0").expect_err("below min should fail");
    assert_eq!(err, "expected unsigned integer >= 1, got '0'");

    let err = parse_input_value(&ty, "-3").expect_err("negative should fail");
    assert!(
        err.starts_with("expected unsigned integer, got '-3'"),
        "got: {err}"
    );
}

#[test]
fn load_dotenv_for_cli_reads_cwd_dotenv_without_overwriting_existing_env() {
    let _guard = env_lock();
    let tmp = tempdir().expect("tempdir");
    let env_path = tmp.path().join(".env");
    std::fs::write(
        &env_path,
        "BACKEND_URL=https://staging-api.example.test\nOPENHUMAN_APP_ENV=staging\n",
    )
    .expect("write .env");

    let original_dir = std::env::current_dir().expect("current dir");
    // Env lock is held by `_guard`; the vars are restored when `_vars` drops.
    let _vars = EnvVarGuard::unset("BACKEND_URL")
        .with("OPENHUMAN_APP_ENV", "production")
        .without("OPENHUMAN_DOTENV_PATH");
    std::env::set_current_dir(tmp.path()).expect("set current dir");

    let result = load_dotenv_for_cli();

    let loaded_backend = std::env::var("BACKEND_URL").ok();
    let loaded_app_env = std::env::var("OPENHUMAN_APP_ENV").ok();

    std::env::set_current_dir(&original_dir).expect("restore current dir");

    result.expect("dotenv load should succeed");
    assert_eq!(
        loaded_backend.as_deref(),
        Some("https://staging-api.example.test")
    );
    assert_eq!(loaded_app_env.as_deref(), Some("production"));
}

// --- `mcp` compile-time gate (#4799) ------------------------------------

/// With the `mcp` feature compiled out, `openhuman mcp` must fail with a
/// diagnostic that names the BUILD as the cause — not a generic
/// "unknown namespace" error.
///
/// Why this matters enough to test: the naive way to gate the CLI is to delete
/// the `"mcp" | "mcp-server"` match arm. That is WRONG — `mcp` would fall
/// through to generic namespace resolution and die with `unknown namespace:
/// mcp`, which reads like the user typo'd a command rather than like a
/// property of this build. Instead `cli.rs` is untouched and the arm resolves
/// to `mcp::server::stub::run_stdio_from_cli`, which bails with the message
/// asserted below. An MCP host (Claude Desktop, Cursor, …) spawning
/// `openhuman mcp` therefore gets a non-zero exit + a one-line reason on
/// stderr instead of hanging on stdout that never speaks JSON-RPC.
#[test]
#[cfg(not(feature = "mcp"))]
fn mcp_subcommand_reports_disabled_build_when_gate_off() {
    let _guard = env_lock();

    let err = crate::core::cli::run_from_cli_args(&["mcp".to_string()])
        .expect_err("`openhuman mcp` must fail when the `mcp` feature is compiled out");
    let msg = err.to_string();

    assert!(
        msg.contains("mcp feature disabled"),
        "error must name the compile-time gate as the cause; got: {msg}"
    );
    assert!(
        msg.contains("--features mcp"),
        "error must tell the user how to get a working build; got: {msg}"
    );
    assert!(
        !msg.contains("unknown namespace"),
        "must NOT degrade into generic namespace resolution — that reads like a typo, \
         not a build fact; got: {msg}"
    );
}

/// The `mcp-server` alias must behave identically to `mcp` — both arms route
/// to the same stub, so neither can silently regress into the fall-through.
#[test]
#[cfg(not(feature = "mcp"))]
fn mcp_server_alias_reports_disabled_build_when_gate_off() {
    let _guard = env_lock();

    let err = crate::core::cli::run_from_cli_args(&["mcp-server".to_string()])
        .expect_err("`openhuman mcp-server` must fail when the `mcp` feature is compiled out");

    assert!(
        err.to_string().contains("mcp feature disabled"),
        "the `mcp-server` alias must give the same build-fact diagnostic as `mcp`"
    );
}

#[test]
fn tui_subcommand_points_to_the_separate_executable() {
    let _guard = env_lock();

    let err = crate::core::cli::run_from_cli_args(&["tui".to_string()])
        .expect_err("`openhuman tui` must point to the standalone terminal client");
    let msg = err.to_string();

    assert!(
        msg.contains("openhuman-tui"),
        "error must name the replacement executable; got: {msg}"
    );
    assert!(
        !msg.contains("unknown namespace"),
        "must NOT degrade into generic namespace resolution — that reads like a typo, \
         not a build fact; got: {msg}"
    );
}

#[test]
fn chat_alias_points_to_the_separate_executable() {
    let _guard = env_lock();

    let err = crate::core::cli::run_from_cli_args(&["chat".to_string()])
        .expect_err("`openhuman chat` must point to the standalone terminal client");

    assert!(
        err.to_string().contains("openhuman-tui"),
        "the `chat` alias must name the replacement executable"
    );
}

// --- the capability gate on the generic namespace path -----------------------
//
// Driven through the pure helpers plus a directly-resolved capability set,
// rather than `run_from_cli_args`: reaching a narrowed set end-to-end needs
// `driver = "null"` in a real `config.toml` under a process-global
// `OPENHUMAN_WORKSPACE`, i.e. env mutation plus disk writes. Same reasoning
// recorded in the M5.4 block of `all_tests.rs`.

/// A real typo must stay a typo — the gate never fires for it, because the
/// unfiltered lookup finds no controller to name a family for.
#[test]
fn unknown_namespace_still_reports_unknown_namespace() {
    let err = super::run_namespace_command(
        "definitely_not_a_namespace",
        &["x".to_string()],
        &grouped_schemas(),
    )
    .expect_err("an unknown namespace must error");
    assert!(err.to_string().contains("unknown namespace"), "{err}");
}

#[test]
fn unknown_function_in_a_live_namespace_still_reports_unknown_function() {
    let grouped = grouped_schemas();
    let namespace = grouped
        .keys()
        .next()
        .expect("at least one namespace is registered")
        .clone();
    let err = super::run_namespace_command(
        &namespace,
        &["definitely_not_a_function".to_string()],
        &grouped,
    )
    .expect_err("an unknown function must error");
    assert!(err.to_string().contains("unknown function"), "{err}");
}

/// With no ambient context nothing is filtered, so the CLI's namespace list is
/// exactly what it was before the gate existed.
#[test]
fn default_build_leaves_the_generic_namespace_path_unchanged() {
    let grouped = grouped_schemas();
    assert!(
        grouped.contains_key("memory"),
        "`memory` must still be listed"
    );
    // `memory_diff` was removed with the `memory-git` gate; it must not come
    // back as a listed namespace.
    assert!(
        !grouped.contains_key("memory_diff"),
        "`memory_diff` was removed and must not be listed"
    );
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|s| s.to_string()).collect()
}

#[test]
fn serve_args_without_flags_leave_every_override_unset() {
    let boot = crate::core::server_launcher::HostBoot::new(1u8);
    let (request, verbose) = parse_serve_args(&[], None, Some(boot.clone()))
        .unwrap()
        .expect("a request");
    assert!(!verbose);
    // Nothing explicit: the host's builder keeps its own listener and services.
    assert_eq!(request.host, None);
    assert_eq!(request.port, None);
    assert!(request.socketio_enabled);
    assert!(!request.headless_api);
    assert_eq!(request.mode, crate::core::runtime::Mode::SingleUser);
    assert_eq!(request.host_boot, Some(boot), "the builder rides along");
}

#[test]
fn serve_args_flags_are_explicit_overrides_on_top_of_the_host_boot() {
    let boot = crate::core::server_launcher::HostBoot::new(1u8);
    let (request, verbose) = parse_serve_args(
        &strings(&["--port", "7801", "--host", "0.0.0.0", "--jsonrpc-only", "-v"]),
        None,
        Some(boot.clone()),
    )
    .unwrap()
    .unwrap();
    assert!(verbose);
    assert_eq!(request.port, Some(7801));
    assert_eq!(request.host.as_deref(), Some("0.0.0.0"));
    assert!(!request.socketio_enabled);
    assert_eq!(request.host_boot, Some(boot));

    let (headless, _) = parse_serve_args(&strings(&["--headless-api"]), None, None)
        .unwrap()
        .unwrap();
    assert!(headless.headless_api && !headless.socketio_enabled);
    assert!(headless.host_boot.is_none());
}

#[test]
fn serve_args_mode_precedence_is_unchanged_by_the_host_boot() {
    let boot = crate::core::server_launcher::HostBoot::new(1u8);
    // The environment can raise a run to SaaS but a flag cannot lower it.
    let err = parse_serve_args(&[], Some("saas"), Some(boot.clone())).unwrap_err();
    assert!(err.to_string().contains("--saas-config"), "{err}");
    let (request, _) = parse_serve_args(
        &strings(&["--mode", "saas", "--saas-config", "/etc/oh.toml"]),
        None,
        Some(boot),
    )
    .unwrap()
    .unwrap();
    assert_eq!(request.mode, crate::core::runtime::Mode::Saas);
}

#[test]
fn serve_args_reject_unknown_flags_and_print_help_without_a_request() {
    assert!(parse_serve_args(&strings(&["--bogus"]), None, None).is_err());
    assert!(parse_serve_args(&strings(&["--help"]), None, None)
        .unwrap()
        .is_none());
}
