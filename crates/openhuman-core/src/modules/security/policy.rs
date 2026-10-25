//! The single translation point for TinySecurity bootstrap policy.

use crate::config::Config;

/// Wire contract served by the checksum-pinned v0.17.1 module. The fork bus
/// contains newer host APIs, but this adapter invokes only this exact subset.
pub const MODULE_CONTRACT_VERSION: (u32, u32) = (1, 28);

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
        ..tinysecurity_bus::ModuleConfig::default()
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
