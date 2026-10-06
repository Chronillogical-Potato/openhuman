//! Docker-backed sandbox execution backend.
//!
//! Policy mapping for the Docker sandbox; the one-shot container itself is
//! `tinybox_docker::OneShot` / `DockerCli`. Commands run inside ephemeral
//! containers with:
//! - Controlled workspace mounts (host `action_dir` → `/workspace`)
//! - Network isolation (default: `none`)
//! - Resource limits (memory, CPU)
//! - Capability dropping (`--cap-drop ALL`)
//! - Read-only rootfs (configurable)
//! - Environment passthrough (explicit allowlist only)
//! - Automatic container cleanup on completion
//!
//! The host core process is never inside the container — only the
//! spawned command runs sandboxed.

use super::types::{
    SandboxBackendHandle, SandboxBackendKind, SandboxExecRequest, SandboxExecResult, SandboxPolicy,
    SandboxStatus,
};
use tinybox_docker::{DockerCli, OneShot};

/// Label applied to all sandbox containers for orphan cleanup.
const CONTAINER_LABEL: &str = "openhuman.sandbox=true";

/// Check whether Docker is available and responsive.
pub async fn is_docker_available() -> bool {
    DockerCli::new().is_available().await
}

/// Execute a command inside an ephemeral Docker container.
///
/// Policy stays here (which image, network, limits and mounts the
/// `SandboxPolicy` asks for); the one-shot `docker run --rm` command line and
/// its execution are `tinybox_docker::{OneShot, DockerCli}`.
pub async fn docker_exec(
    policy: &SandboxPolicy,
    request: &SandboxExecRequest,
) -> anyhow::Result<SandboxExecResult> {
    let overrides = policy.docker_overrides.as_ref();
    let mut spec = OneShot::new(&policy.workspace_root, &request.command)
        .with_label(CONTAINER_LABEL)
        .with_read_only_mounts(policy.read_only_mounts.iter().cloned());
    if let Some(ov) = overrides {
        if let Some(image) = &ov.image {
            spec = spec.with_image(image);
        }
        if let Some(network) = &ov.network {
            spec = spec.with_network(network);
        }
        if let Some(mb) = ov.memory_limit_mb {
            spec = spec.with_memory_mb(mb);
        }
        if let Some(cpu) = ov.cpu_limit {
            spec = spec.with_cpus(cpu);
        }
        if let Some(ro) = ov.read_only_rootfs {
            spec = spec.with_read_only_rootfs(ro);
        }
        spec = spec.with_extra_cap_drops(ov.extra_caps_drop.iter().cloned());
    }

    // Environment passthrough (explicit allowlist, only when set), then the
    // request-specific environment.
    let mut env: Vec<(std::ffi::OsString, std::ffi::OsString)> = policy
        .env_passthrough
        .iter()
        .filter_map(|name| std::env::var(name).ok().map(|v| (name.into(), v.into())))
        .collect();
    env.extend(request.env.iter().map(|(k, v)| (k.clone(), v.clone())));
    let spec = spec.with_env(env);

    tracing::debug!(
        workspace = %policy.workspace_root.display(),
        command = %request.command,
        "[sandbox:docker] launching container"
    );

    match DockerCli::new().run_one_shot(&spec, request.timeout).await {
        Ok(out) if out.timed_out => {
            tracing::warn!(
                timeout_secs = request.timeout.as_secs(),
                "[sandbox:docker] container timed out, killing"
            );
            Ok(SandboxExecResult {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
                timed_out: true,
            })
        }
        Ok(out) => {
            tracing::debug!(
                exit_code = out.exit_code,
                stdout_len = out.stdout.len(),
                stderr_len = out.stderr.len(),
                "[sandbox:docker] container exited"
            );
            Ok(SandboxExecResult {
                exit_code: out.exit_code,
                stdout: out.stdout,
                stderr: out.stderr,
                timed_out: false,
            })
        }
        Err(e) => {
            tracing::error!(error = %e, "[sandbox:docker] failed to spawn container");
            anyhow::bail!("Docker execution failed: {e}")
        }
    }
}

/// Clean up orphaned sandbox containers (those labeled with
/// `openhuman.sandbox=true` that are still running).
pub async fn cleanup_orphaned_containers() -> anyhow::Result<u32> {
    let count = DockerCli::new().kill_labelled(CONTAINER_LABEL).await?;
    tracing::debug!(count = count, "[sandbox:docker] orphan cleanup finished");
    Ok(count)
}

/// Create a handle representing the Docker backend state.
pub async fn docker_backend_handle() -> SandboxBackendHandle {
    let available = is_docker_available().await;
    SandboxBackendHandle {
        kind: SandboxBackendKind::Docker,
        status: if available {
            SandboxStatus::Ready
        } else {
            SandboxStatus::Error
        },
        backend_id: None,
    }
}

/// Validate that a sandbox policy's Docker configuration doesn't have
/// dangerous settings (host network, privileged mounts, etc.).
pub fn validate_docker_policy(policy: &SandboxPolicy) -> Result<(), Vec<String>> {
    let mut issues = Vec::new();

    if let Some(overrides) = &policy.docker_overrides {
        if overrides.network.as_deref() == Some("host") {
            issues.push("Docker sandbox uses host network — defeats network isolation".into());
        }
    }

    // Check for dangerous mount paths.
    let dangerous_mounts = ["/", "/etc", "/var/run/docker.sock", "/proc", "/sys"];
    for mount in &policy.read_only_mounts {
        let path_str = mount.to_string_lossy();
        for &dangerous in &dangerous_mounts {
            if path_str == dangerous {
                issues.push(format!(
                    "Dangerous read-only mount: {path_str} — could leak host secrets"
                ));
            }
        }
    }

    let workspace_str = policy.workspace_root.to_string_lossy();
    for &dangerous in &dangerous_mounts {
        if workspace_str == dangerous {
            issues.push(format!(
                "Workspace root is a dangerous path: {workspace_str}"
            ));
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

#[cfg(test)]
#[path = "docker_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "docker_exec_tests.rs"]
mod exec_tests;
