//! The single translation point for TinySecurity bootstrap policy.

use crate::config::Config;

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
    trusted_host: String,
) -> tinysecurity_bus::ModuleConfig {
    tinysecurity_bus::ModuleConfig {
        trusted_host: Some(trusted_host),
        ..module_config(config)
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
