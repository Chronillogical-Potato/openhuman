use super::policy::{apply_requested_backend, config_disables_sandbox, sandbox_disabled_by_host};
use crate::core::runtime::DispatchCtx;
use crate::sandbox::{
    docker,
    shell_exec::{ComeBack, ShellRunOutcome},
    types::{SandboxBackendKind, SandboxExecRequest, SandboxExecResult, SandboxPolicy},
};
use std::{collections::HashMap, ffi::OsString, path::Path, time::Duration};

/// Execute a command through the appropriate sandbox backend.
///
/// Returns the sandboxed execution result. The caller (typically the
/// shell tool) is responsible for converting this into a `ToolResult`.
pub async fn execute_in_sandbox(
    ctx: &DispatchCtx,
    policy: &SandboxPolicy,
    command: &str,
    working_dir: &Path,
    extra_env: HashMap<OsString, OsString>,
    timeout: Duration,
) -> anyhow::Result<SandboxExecResult> {
    execute_in_sandbox_metered(ctx, policy, command, working_dir, extra_env, timeout, None).await
}

/// [`execute_in_sandbox`] that feeds `meter` with the command's output as it
/// streams (the native backends; Docker reports elapsed time only), for the
/// shell tool's progress line.
pub(crate) async fn execute_in_sandbox_metered(
    ctx: &DispatchCtx,
    policy: &SandboxPolicy,
    command: &str,
    working_dir: &Path,
    extra_env: HashMap<OsString, OsString>,
    timeout: Duration,
    meter: Option<crate::sandbox::shell_progress::ShellOutputMeter>,
) -> anyhow::Result<SandboxExecResult> {
    let outcome = execute_in_sandbox_yielding(
        ctx,
        policy,
        command,
        working_dir,
        extra_env,
        timeout,
        ComeBack::default(),
        meter,
    )
    .await?;
    match outcome {
        ShellRunOutcome::Finished(result) => Ok(result),
        ShellRunOutcome::Running(_) => {
            anyhow::bail!("a sandboxed run without a yield point returned while running")
        }
    }
}

/// [`execute_in_sandbox_metered`] for a call that may come back first: on the
/// native backends a command still running at `come_back.yield_after` is
/// handed back as [`ShellRunOutcome::Running`]; Docker cannot leave a command
/// running, so there `come_back.deadline_instead` bounds it.
pub(crate) async fn execute_in_sandbox_yielding(
    ctx: &DispatchCtx,
    policy: &SandboxPolicy,
    command: &str,
    working_dir: &Path,
    extra_env: HashMap<OsString, OsString>,
    timeout: Duration,
    come_back: ComeBack,
    meter: Option<crate::sandbox::shell_progress::ShellOutputMeter>,
) -> anyhow::Result<ShellRunOutcome> {
    // The saved backend is an operator's explicit isolation requirement. It
    // must be checked at the spawn boundary, including callers that resolved
    // their policy from an older or default RuntimeConfig.
    let mut effective_policy = policy.clone();
    if !crate::core::runtime::is_saas() && sandbox_disabled_by_host(ctx) {
        effective_policy.backend = SandboxBackendKind::None;
    }
    let mut module_config = None;
    let explicitly_requested = if crate::core::runtime::is_saas() || sandbox_disabled_by_host(ctx) {
        false
    } else {
        let config = ctx
            .load_config()
            .await
            .map_err(|e| anyhow::anyhow!("Cannot read sandbox configuration: {e}"))?;
        module_config = Some(config.clone());
        if config_disables_sandbox(&config) {
            false
        } else {
            apply_requested_backend(
                &mut effective_policy,
                &config.sandbox,
                &config.runtime,
                &config,
            )
            .await?
        }
    };
    let policy = &effective_policy;
    // Validate the working directory up front so a missing/bad action_dir
    // surfaces an actionable, path-naming error here rather than an opaque OS
    // error 267 (ERROR_DIRECTORY) at spawn time — parity with the unsandboxed
    // `NativeRuntime::build_shell_command` guard. (#3353, Fix 2)
    //
    // The validation is host-side, so it is applied per-backend: for None/Local
    // `working_dir` *is* a host path; for Docker `working_dir` is the
    // container-side mount target (e.g. `/workspace`) which must NOT be
    // stat'd/created on the host — there we validate the host-side mount source
    // (`policy.workspace_root`) instead.
    match policy.backend {
        SandboxBackendKind::None => {
            crate::config::ensure_usable_cwd(working_dir)?;
            let config = match module_config {
                Some(config) => config,
                None => ctx.load_config().await.map_err(|error| {
                    anyhow::anyhow!("Cannot read module configuration: {error}")
                })?,
            };
            execute_unsandboxed(
                ctx,
                &config,
                policy,
                command,
                working_dir,
                &extra_env,
                timeout,
                come_back.yield_after,
                meter,
            )
            .await
        }
        SandboxBackendKind::Local => {
            crate::config::ensure_usable_cwd(working_dir)?;
            let config = match module_config {
                Some(config) => config,
                None => ctx.load_config().await.map_err(|error| {
                    anyhow::anyhow!("Cannot read module configuration: {error}")
                })?,
            };
            let policy =
                crate::sandbox::local_grants::with_resolved_grants(&config, policy).await?;
            crate::sandbox::native_jail::execute(
                ctx,
                &config,
                &policy,
                command,
                working_dir,
                &extra_env,
                timeout,
                explicitly_requested,
                come_back.yield_after,
                meter,
            )
            .await
        }
        SandboxBackendKind::Docker => {
            let timeout = come_back
                .deadline_instead
                .map_or(timeout, |deadline| timeout.min(deadline));
            crate::config::ensure_usable_cwd(&policy.workspace_root)?;
            let request = SandboxExecRequest {
                command: command.to_string(),
                working_dir: working_dir.to_path_buf(),
                env: extra_env,
                timeout,
            };
            let config = match module_config {
                Some(config) => config,
                None => ctx.load_config().await.map_err(|error| {
                    anyhow::anyhow!("Cannot read module configuration: {error}")
                })?,
            };
            docker::docker_exec(&config, policy, &request)
                .await
                .map(ShellRunOutcome::Finished)
        }
    }
}

/// Passthrough execution with no sandbox (for `SandboxBackendKind::None`).
///
/// TinyBox's `StartNativeShellExec` selects the shell (so Windows gets
/// `cmd.exe`, #4705), clears the environment down to its safe snapshot (the
/// `SANDBOX_ENV_PASSTHROUGH` names plus the Windows process-bootstrap
/// variables), applies the caller's overrides and owns the scratch directory.
pub(crate) async fn execute_unsandboxed(
    ctx: &DispatchCtx,
    config: &crate::config::Config,
    policy: &SandboxPolicy,
    command: &str,
    working_dir: &Path,
    extra_env: &HashMap<OsString, OsString>,
    timeout: Duration,
    yield_after: Option<Duration>,
    meter: Option<crate::sandbox::shell_progress::ShellOutputMeter>,
) -> anyhow::Result<ShellRunOutcome> {
    let mut env = std::collections::BTreeMap::new();
    for (name, value) in extra_env {
        let (Some(name), Some(value)) = (name.to_str(), value.to_str()) else {
            anyhow::bail!("sandbox environment is not UTF-8");
        };
        env.insert(name.to_owned(), value.to_owned());
    }
    std::fs::create_dir_all(&policy.state_dir)
        .map_err(|error| anyhow::anyhow!("Cannot prepare sandbox state directory: {error}"))?;
    let mut run = crate::sandbox::shell_exec::NativeShellRun::new(
        command,
        working_dir,
        &policy.state_dir,
        timeout,
    );
    run.meter = meter;
    run.env = env;
    run.yield_after = yield_after;
    crate::sandbox::shell_exec::run_yielding(ctx, config, run).await
}
