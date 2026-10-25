//! Sandbox policy and backend-status characterization for issue #646.
//! These tests pin the current host policy resolution, command precedence,
//! environment pass-through allowlist, and status mapping without starting a
//! real sandbox backend.

#[path = "support/env_guard.rs"]
mod env_guard;
#[path = "support/tinyhumans_boot.rs"]
mod tinyhumans_boot;

use std::path::Path;

use tinyagents_definition::SandboxMode;

use openhuman_core::{
    config::{Config, RuntimeConfig, SandboxBackend},
    core::runtime::DispatchCtx,
    sandbox::{command_requires_sandbox, resolve_sandbox_policy, SandboxBackendKind},
};

const ACTION: &str = "/work/action";
const STATE: &str = "/work/state";

fn resolve(runtime: &RuntimeConfig, remote: bool) -> openhuman_core::sandbox::SandboxPolicy {
    resolve_sandbox_policy(
        &DispatchCtx::process_default(),
        SandboxMode::Sandboxed,
        Path::new(ACTION),
        Path::new(STATE),
        runtime,
        remote,
    )
}

async fn scratch_config(
    backend: SandboxBackend,
    enabled: Option<bool>,
) -> (tempfile::TempDir, Config) {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    config.config_path = dir.path().join("config.toml");
    config.action_dir = dir.path().join("action");
    config.sandbox.backend = backend;
    config.sandbox.enabled = enabled;
    config.save().await.unwrap();
    (dir, config)
}

#[test]
fn resolver_selects_executor_and_network_policy_for_the_turn_mode() {
    let local = RuntimeConfig::default();
    let docker = RuntimeConfig {
        kind: "docker".into(),
        ..RuntimeConfig::default()
    };
    // (runtime, remote) -> (named executor, allow_network)
    let cases = [
        (&local, false, SandboxBackendKind::Local, true),
        (&local, true, SandboxBackendKind::Docker, false),
        (&docker, false, SandboxBackendKind::Docker, true),
        (&docker, true, SandboxBackendKind::Docker, false),
    ];
    for (runtime, remote, backend, network) in cases {
        let policy = resolve(runtime, remote);
        let label = format!("kind={} remote={remote}", runtime.kind);
        assert_eq!(policy.backend, backend, "{label}");
        assert_eq!(policy.allow_network, network, "{label}");
        assert_eq!(
            policy.docker_overrides.is_some(),
            backend == SandboxBackendKind::Docker,
            "{label}"
        );
        assert_eq!(policy.workspace_root, Path::new(ACTION), "{label}");
        assert_eq!(policy.state_dir, Path::new(STATE), "{label}");
        assert!(
            policy.read_only_mounts.is_empty() && policy.read_write_mounts.is_empty(),
            "{label}"
        );
    }
}

#[test]
fn env_passthrough_is_the_pinned_safe_allowlist() {
    let policy = resolve(&RuntimeConfig::default(), false);
    assert_eq!(
        policy.env_passthrough,
        ["PATH", "HOME", "TERM", "LANG", "LC_ALL", "LC_CTYPE", "USER", "SHELL", "TMPDIR"]
    );
    for secret in [
        "AWS_SECRET_ACCESS_KEY",
        "OPENHUMAN_CORE_TOKEN",
        "GITHUB_TOKEN",
    ] {
        assert!(
            !policy.env_passthrough.iter().any(|v| v == secret),
            "{secret}"
        );
    }
}

#[tokio::test]
async fn command_requires_sandbox_precedence_for_single_user_hosts() {
    let _lock = env_guard::env_lock_async().await;
    tinyhumans_boot::boot();
    let _keyring = env_guard::EnvVarGuard::set("OPENHUMAN_KEYRING_BACKEND", "file");
    let _storage = env_guard::EnvVarGuard::unset("OPENHUMAN_STORAGE_URL");

    // Default config: only an agent that asks for a sandbox gets one.
    let (dir, _config) = scratch_config(SandboxBackend::Auto, None).await;
    let _ws = env_guard::EnvVarGuard::set_path("OPENHUMAN_WORKSPACE", dir.path());
    {
        let _off = env_guard::EnvVarGuard::unset("OPENHUMAN_SANDBOX");
        assert!(
            !command_requires_sandbox(&DispatchCtx::process_default(), None)
                .await
                .unwrap()
        );
        assert!(!command_requires_sandbox(
            &DispatchCtx::process_default(),
            Some(SandboxMode::None)
        )
        .await
        .unwrap());
        assert!(!command_requires_sandbox(
            &DispatchCtx::process_default(),
            Some(SandboxMode::ReadOnly)
        )
        .await
        .unwrap());
        assert!(command_requires_sandbox(
            &DispatchCtx::process_default(),
            Some(SandboxMode::Sandboxed)
        )
        .await
        .unwrap());
    }
    // The host-level off switch short-circuits the sandbox requirement.
    {
        let _off = env_guard::EnvVarGuard::set("OPENHUMAN_SANDBOX", "off");
        assert!(!command_requires_sandbox(
            &DispatchCtx::process_default(),
            Some(SandboxMode::Sandboxed)
        )
        .await
        .unwrap());
    }
    drop(_ws);
    drop(dir);

    // An explicit operator backend requires a sandbox even for a default agent...
    for backend in [SandboxBackend::Landlock, SandboxBackend::Docker] {
        let label = format!("{backend:?}");
        let (dir, _config) = scratch_config(backend, None).await;
        let _ws = env_guard::EnvVarGuard::set_path("OPENHUMAN_WORKSPACE", dir.path());
        let _off = env_guard::EnvVarGuard::unset("OPENHUMAN_SANDBOX");
        assert!(
            command_requires_sandbox(&DispatchCtx::process_default(), None)
                .await
                .unwrap(),
            "{label}"
        );
        assert!(
            command_requires_sandbox(&DispatchCtx::process_default(), Some(SandboxMode::None))
                .await
                .unwrap(),
            "{label}"
        );
    }

    // The legacy `enabled = false` flag also disables sandbox requirements.
    let (dir, _config) = scratch_config(SandboxBackend::Docker, Some(false)).await;
    let _ws = env_guard::EnvVarGuard::set_path("OPENHUMAN_WORKSPACE", dir.path());
    let _off = env_guard::EnvVarGuard::unset("OPENHUMAN_SANDBOX");
    assert!(
        !command_requires_sandbox(&DispatchCtx::process_default(), None)
            .await
            .unwrap()
    );
}

// Backend availability depends on the host OS and installed executors. This
// characterization stays on the pure resolver and never starts or probes one.
