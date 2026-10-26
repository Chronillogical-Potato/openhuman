//! The single translation point for TinySecurity bootstrap policy.

use crate::config::Config;

/// Wire contract served by the checksum-pinned v0.18.0 module. The fork bus
/// contains newer host APIs, but this adapter invokes only this exact subset.
pub const MODULE_CONTRACT_VERSION: (u32, u32) = (1, 33);

/// Check the exact published artifact contract this adapter was built for.
#[must_use]
pub const fn supports_contract(version: (u32, u32)) -> bool {
    version.0 == MODULE_CONTRACT_VERSION.0 && version.1 == MODULE_CONTRACT_VERSION.1
}

/// Translate OpenHuman autonomy settings to the published bus contract.
#[must_use]
pub fn policy_config(config: &Config) -> tinysecurity_bus::PolicyConfig {
    tinysecurity_bus::PolicyConfig {
        enabled: config.autonomy.enabled,
        action_dir: config.action_dir.display().to_string(),
        workspace_dir: config.workspace_dir.display().to_string(),
        // These settings are inert while autonomy is disabled. Do not send
        // host defaults that the module must reject despite having no effect.
        allowed_commands: if config.autonomy.enabled {
            config.autonomy.allowed_commands.clone()
        } else {
            Vec::new()
        },
        forbidden_paths: if config.autonomy.enabled {
            config.autonomy.forbidden_paths.clone()
        } else {
            Vec::new()
        },
    }
}

/// Reject enabled host policy that the published module contract cannot
/// represent. Disabled policy settings are inert and serialize empty above.
pub fn validate(config: &Config) -> Result<(), &'static str> {
    if !config.autonomy.enabled {
        return Ok(());
    }
    if !config.action_dir.is_absolute() || !config.workspace_dir.is_absolute() {
        return Err("enabled policy roots must be absolute paths");
    }
    if config
        .autonomy
        .allowed_commands
        .iter()
        .any(|command| command.is_empty() || command.chars().any(char::is_whitespace))
    {
        return Err("enabled policy allows only argument-free command names");
    }
    if config
        .autonomy
        .forbidden_paths
        .iter()
        .any(|path| !std::path::Path::new(path).is_absolute())
    {
        return Err("enabled policy forbidden paths must be absolute");
    }
    Ok(())
}

/// Complete SDK bootstrap payload used both by first load and reinitialization.
#[must_use]
pub fn module_config(config: &Config) -> tinysecurity_bus::ModuleConfig {
    tinysecurity_bus::ModuleConfig {
        policy: policy_config(config),
        approvals: Some(approval_init_config()),
        audit: config.audit.enabled.then(|| audit_init_config(config)),
        ..tinysecurity_bus::ModuleConfig::default()
    }
}

const AUDIT_CALLBACK_TIMEOUT_MS: u32 = 20_000;

/// Init-time subject is deliberately separate from every registered user
/// subject. Real audit rows are sent through `RecordAudit2` to the per-subject
/// stream registered by `security::approval::native`.
fn bootstrap_subject() -> tinysecurity_bus::SubjectScope {
    tinysecurity_bus::SubjectScope {
        user: "openhuman".into(),
        workspace: "bootstrap".into(),
        agent: "bootstrap".into(),
    }
}

fn approval_init_config() -> tinysecurity_bus::ApprovalConfig2 {
    tinysecurity_bus::ApprovalConfig2 {
        timeout_ms: AUDIT_CALLBACK_TIMEOUT_MS,
        scopes: vec![tinysecurity_bus::ApprovalScope2 {
            scope_id: "0".repeat(32),
            subject: bootstrap_subject(),
            policy_ref: "openhuman/approval/bootstrap".into(),
            generation: 1,
            ceiling: tinysecurity_bus::AccessTier::Read,
            require_approval: true,
            auto_approve_all: false,
        }],
    }
}

fn audit_init_config(config: &Config) -> tinysecurity_bus::AuditConfig2 {
    let max_bytes = u64::from(config.audit.max_size_mb)
        .saturating_mul(1024 * 1024)
        .max(1);
    tinysecurity_bus::AuditConfig2 {
        scope: bootstrap_subject(),
        stream_id: "b".repeat(32),
        timeout_ms: AUDIT_CALLBACK_TIMEOUT_MS,
        required: false,
        native_path: None,
        max_bytes,
        max_archives: 3,
        journal_max_bytes: max_bytes,
        journal_max_events: 10_000,
    }
}

/// Complete SDK bootstrap payload with the host identity required by the
/// module's authenticated bus boundary.
pub fn module_config_for_host(
    config: &Config,
    trusted_host: Option<String>,
) -> Result<tinysecurity_bus::ModuleConfig, &'static str> {
    let trusted_host = trusted_host.ok_or("module host identity unavailable")?;
    Ok(tinysecurity_bus::ModuleConfig {
        contract_version: MODULE_CONTRACT_VERSION,
        trusted_host: Some(trusted_host),
        ..module_config(config)
    })
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
