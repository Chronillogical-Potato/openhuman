use crate::config::{RuntimeConfig, SandboxBackend, SandboxConfig};
use crate::core::runtime::DispatchCtx;
use std::path::Path;

use crate::sandbox::{
    docker,
    types::{SandboxBackendHandle, SandboxBackendKind, SandboxPolicy, SandboxStatus},
};
use tinyagents_definition::SandboxMode;

/// Safe environment variables forwarded into sandboxed execution.
///
/// This is the Docker container's host-environment allowlist and the set the
/// TinyBox module's own safe snapshot starts from for every native shell child
/// ([`execute_unsandboxed`] and the native jail adapter): the module reads these
/// names from the parent and adds the Windows process-bootstrap variables
/// itself. Keeping the bootstrap names out of this policy prevents Windows paths
/// from being passed into Linux Docker containers.
pub const SANDBOX_ENV_PASSTHROUGH: &[&str] = &[
    "PATH", "HOME", "TERM", "LANG", "LC_ALL", "LC_CTYPE", "USER", "SHELL", "TMPDIR",
];

/// Host switch that turns the agent sandbox off for the whole process.
///
/// For hosts that already isolate the core (a container, a CI or benchmark
/// task image, a VM): the OS jail confines commands to the action dir, which
/// breaks work such as installing packages or editing `/etc` that the outer
/// isolation already permits. `off`, `none`, `0`, `false` or `disabled`
/// (case-insensitive) disable it; anything else, or unset, leaves it on.
pub const SANDBOX_OFF_ENV: &str = "OPENHUMAN_SANDBOX";

/// Whether `value` (the `OPENHUMAN_SANDBOX` setting) disables the sandbox.
pub fn sandbox_off_value(value: Option<&str>) -> bool {
    matches!(
        value.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("off" | "none" | "0" | "false" | "disabled")
    )
}

/// Whether a command tool must use the sandbox execution path. An explicit
/// backend in the active config is an operator requirement even when the
/// agent definition uses the usual `SandboxMode::None` default.
pub async fn command_requires_sandbox(
    ctx: &DispatchCtx,
    sandbox_mode: Option<SandboxMode>,
) -> Result<bool, String> {
    if crate::core::runtime::is_saas() {
        // Every SaaS command must enter the per-user container path, including
        // language tools whose agent definition does not request a sandbox.
        return Ok(true);
    }
    if sandbox_disabled_by_host(ctx) {
        return Ok(false);
    }
    if matches!(sandbox_mode, Some(SandboxMode::Sandboxed)) {
        return Ok(true);
    }
    let config = ctx.load_config().await?;
    if config_disables_sandbox(&config) {
        return Ok(false);
    }
    Ok(matches!(
        config.sandbox.backend,
        SandboxBackend::Landlock
            | SandboxBackend::Firejail
            | SandboxBackend::Bubblewrap
            | SandboxBackend::Docker
    ))
}

/// Resolve the policy for a command tool before building its command line.
/// SaaS always uses the user's container; other hosts apply the operator's
/// backend selection while retaining an agent-level sandbox requirement.
pub(crate) async fn resolve_command_policy(
    ctx: &DispatchCtx,
    action_dir: &Path,
    state_dir: &Path,
) -> Result<SandboxPolicy, String> {
    if crate::core::runtime::is_saas() {
        return crate::profiles::tools::sandbox_policy(action_dir, state_dir);
    }
    let config = ctx
        .load_config()
        .await
        .map_err(|error| format!("Cannot read sandbox configuration: {error}"))?;
    let mut policy = resolve_sandbox_policy(
        ctx,
        SandboxMode::Sandboxed,
        action_dir,
        state_dir,
        &config.runtime,
        false,
    );
    if !config_disables_sandbox(&config) && !sandbox_disabled_by_host(ctx) {
        apply_requested_backend(&mut policy, &config.sandbox, &config.runtime, &config)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(policy)
}

pub(crate) fn sandbox_disabled_by_host(ctx: &DispatchCtx) -> bool {
    sandbox_off_value(std::env::var(SANDBOX_OFF_ENV).ok().as_deref()) || sandbox_component_off(ctx)
}

/// `[components] sandbox = false` in the context `ctx` runs under. With no
/// context the component is on, since the set only narrows.
pub(crate) fn sandbox_component_off(ctx: &DispatchCtx) -> bool {
    let off = !ctx.components().sandbox;
    if off {
        tracing::debug!("[sandbox] component 'sandbox' disabled, using the unsandboxed fallback");
    }
    off
}

/// The saved config says no isolation: the legacy `enabled = false` flag or
/// `[components] sandbox = false`.
pub(crate) fn config_disables_sandbox(config: &crate::config::Config) -> bool {
    config.sandbox.enabled == Some(false)
        || !crate::core::runtime::ComponentSet::from_config(&config.components).sandbox
}

/// Resolve a `SandboxPolicy` from the agent's `SandboxMode`, the
/// session origin, and the global runtime config.
///
/// Non-main sessions (channel, cron, remote) default to `Docker` when
/// the mode is `Sandboxed` and Docker is configured. Local interactive
/// sessions default to `Local` (the TinyBox module's native process jail).
///
/// `state_dir` is the core's internal `workspace_dir`; host-side scratch such
/// as output capture goes there instead of into `action_dir`.
pub fn resolve_sandbox_policy(
    ctx: &DispatchCtx,
    mode: SandboxMode,
    action_dir: &Path,
    state_dir: &Path,
    runtime_config: &RuntimeConfig,
    is_remote_session: bool,
) -> SandboxPolicy {
    let backend = match mode {
        SandboxMode::None => SandboxBackendKind::None,
        SandboxMode::ReadOnly => SandboxBackendKind::None,
        SandboxMode::Sandboxed if sandbox_disabled_by_host(ctx) => {
            tracing::debug!(
                env = SANDBOX_OFF_ENV,
                "[sandbox] host disabled the sandbox; running sandboxed mode unconfined"
            );
            SandboxBackendKind::None
        }
        SandboxMode::Sandboxed => {
            if runtime_config.kind == "docker" || is_remote_session {
                SandboxBackendKind::Docker
            } else {
                SandboxBackendKind::Local
            }
        }
    };

    let docker_overrides = if backend == SandboxBackendKind::Docker {
        let dc = &runtime_config.docker;
        Some(crate::sandbox::types::DockerOverrides {
            image: Some(dc.image.clone()),
            network: Some(dc.network.clone()),
            memory_limit_mb: dc.memory_limit_mb,
            cpu_limit: dc.cpu_limit,
            read_only_rootfs: Some(dc.read_only_rootfs),
            extra_caps_drop: vec![],
        })
    } else {
        None
    };

    let allow_network = match mode {
        SandboxMode::Sandboxed => !is_remote_session,
        _ => true,
    };

    // The jail denies whatever it is not told about; the toolchain/git grants
    // everyday commands need are resolved by TinyBox at spawn time
    // (`sandbox::local_grants`), so the policy starts without mounts.
    tracing::debug!(
        mode = ?mode,
        backend = ?backend,
        is_remote = is_remote_session,
        action_dir = %action_dir.display(),
        "[sandbox] resolved policy"
    );

    SandboxPolicy {
        backend,
        workspace_root: action_dir.to_path_buf(),
        state_dir: state_dir.to_path_buf(),
        read_only_mounts: Vec::new(),
        read_write_mounts: Vec::new(),
        allow_network,
        env_passthrough: SANDBOX_ENV_PASSTHROUGH
            .iter()
            .map(|s| s.to_string())
            .collect(),
        docker_overrides,
    }
}

/// Create a backend handle for the resolved policy. For Docker this
/// checks availability; for Local it checks the OS backend.
/// The status a `Local` sandbox handle reports, given the backend `pick_backend`
/// actually chose.
///
/// Extracted as a free function so the decision is testable on **any** host.
/// Asserting it through `create_sandbox_backend` cannot work: which branch runs
/// depends on whether the machine has an OS jail, so on a host with Seatbelt or
/// Landlock the noop path is never reached and a regression to "always Ready"
/// passes unnoticed. That is exactly how the original defect survived.
#[cfg(any(feature = "modules", test))]
pub(crate) fn local_status_for_jail(
    isolation: Option<crate::sandbox::types::SandboxIsolation>,
) -> SandboxStatus {
    if isolation.is_some() {
        SandboxStatus::Ready
    } else {
        SandboxStatus::Inactive
    }
}

pub async fn create_sandbox_backend(
    ctx: &DispatchCtx,
    policy: &SandboxPolicy,
) -> SandboxBackendHandle {
    if policy.backend != SandboxBackendKind::Docker {
        return create_sandbox_backend_with_config(policy, &crate::config::Config::default())
            .await
            .unwrap_or(SandboxBackendHandle {
                kind: policy.backend,
                status: SandboxStatus::Error,
                backend_id: None,
                isolation: None,
            });
    }
    let config = match ctx.load_config().await {
        Ok(config) => config,
        Err(_) if policy.backend == SandboxBackendKind::Docker => {
            return docker_error_handle();
        }
        Err(_) => crate::config::Config::default(),
    };
    create_sandbox_backend_with_config(policy, &config)
        .await
        .unwrap_or_else(|_| docker_error_handle())
}

/// Resolve backend status with the caller's loaded configuration.
pub async fn create_sandbox_backend_with_config(
    policy: &SandboxPolicy,
    config: &crate::config::Config,
) -> Result<SandboxBackendHandle, String> {
    match policy.backend {
        SandboxBackendKind::None => Ok(SandboxBackendHandle {
            kind: SandboxBackendKind::None,
            status: SandboxStatus::Ready,
            backend_id: None,
            isolation: None,
        }),
        SandboxBackendKind::Local => {
            let (backend_name, status, isolation) =
                crate::sandbox::native_jail::status(config).await?;
            if status == SandboxStatus::Inactive {
                tracing::warn!(
                    backend = backend_name,
                    "[sandbox:local] TinyBox native jail is unavailable or below the required isolation floor"
                );
            }
            Ok(SandboxBackendHandle {
                kind: SandboxBackendKind::Local,
                status,
                backend_id: Some(backend_name),
                isolation,
            })
        }
        SandboxBackendKind::Docker => docker::docker_backend_handle(config).await,
    }
}

fn docker_error_handle() -> SandboxBackendHandle {
    SandboxBackendHandle {
        kind: SandboxBackendKind::Docker,
        status: SandboxStatus::Error,
        backend_id: None,
        isolation: None,
    }
}

/// Apply an operator-selected backend. Auto retains the caller's existing
/// policy. None opts out only when no agent-level sandbox is required.
pub(crate) async fn apply_requested_backend(
    policy: &mut SandboxPolicy,
    sandbox: &SandboxConfig,
    runtime: &RuntimeConfig,
    config: &crate::config::Config,
) -> anyhow::Result<bool> {
    match &sandbox.backend {
        SandboxBackend::Auto => Ok(false),
        SandboxBackend::None => Ok(false),
        SandboxBackend::Docker => {
            if policy.backend == SandboxBackendKind::Local {
                policy.read_only_mounts.clear();
                policy.read_write_mounts.clear();
            }
            policy.backend = SandboxBackendKind::Docker;
            let dc = &runtime.docker;
            policy.docker_overrides = Some(crate::sandbox::types::DockerOverrides {
                image: Some(dc.image.clone()),
                network: Some(dc.network.clone()),
                memory_limit_mb: dc.memory_limit_mb,
                cpu_limit: dc.cpu_limit,
                read_only_rootfs: Some(dc.read_only_rootfs),
                extra_caps_drop: vec![],
            });
            Ok(true)
        }
        SandboxBackend::Landlock => {
            crate::sandbox::native_jail::require_backend(config, "landlock").await?;
            policy.backend = SandboxBackendKind::Local;
            policy.docker_overrides = None;
            Ok(true)
        }
        SandboxBackend::Firejail => {
            anyhow::bail!(
                "Firejail sandbox was explicitly requested but this build has no Firejail execution backend"
            )
        }
        SandboxBackend::Bubblewrap => {
            anyhow::bail!(
                "Bubblewrap sandbox was explicitly requested but this build has no Bubblewrap execution backend"
            )
        }
    }
}
