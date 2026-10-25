//! The single translation point for TinySecurity bootstrap policy.

use crate::config::Config;

/// Wire contract served by the checksum-pinned v0.17.1 module. The fork bus
/// contains newer host APIs, but this adapter invokes only this exact subset.
pub const MODULE_CONTRACT_VERSION: (u32, u32) = (1, 28);

/// Check the exact published artifact contract this adapter was built for.
#[must_use]
pub const fn supports_contract(version: (u32, u32)) -> bool {
    version == MODULE_CONTRACT_VERSION
}

/// Translate OpenHuman autonomy settings to the published bus contract.
#[must_use]
pub fn policy_config(config: &Config) -> tinysecurity_bus::PolicyConfig {
    tinysecurity_bus::PolicyConfig {
        enabled: config.autonomy.enabled,
        action_dir: config.action_dir.display().to_string(),
        workspace_dir: config.workspace_dir.display().to_string(),
        allowed_commands: config.autonomy.allowed_commands.clone(),
        forbidden_paths: config.autonomy.forbidden_paths.clone(),
    }
}

/// Complete SDK bootstrap payload used both by first load and reinitialization.
#[must_use]
pub fn module_config(config: &Config) -> tinysecurity_bus::ModuleConfig {
    tinysecurity_bus::ModuleConfig {
        policy: policy_config(config),
        ..tinysecurity_bus::ModuleConfig::default()
    }
}

/// Complete SDK bootstrap payload with the host identity required by the
/// module's authenticated bus boundary.
#[must_use]
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
