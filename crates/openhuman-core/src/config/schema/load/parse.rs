//! The one place a `config.toml` document becomes a [`Config`].
//!
//! Deserializing the config tree straight from TOML instantiates every derived
//! `Deserialize` impl in it once per `toml` map-access type (about six), and the
//! tree has well over a hundred types. Parsing into a [`toml::Value`] first and
//! decoding that through [`serde_json::Value`] keeps one decoder instantiation
//! per type. Syntax errors still carry TOML line and column; a type error names
//! the dotted field path (`agent.agent_timeout_secs: invalid type: ...`) through
//! `serde_path_to_error`, though not the line.

use crate::config::Config;

/// Parse a `config.toml` document into a [`Config`].
#[inline(never)]
pub(crate) fn config_from_toml_str(contents: &str) -> anyhow::Result<Config> {
    let document: toml::Value = toml::from_str(contents)?;
    let json = serde_json::to_value(document)?;
    serde_path_to_error::deserialize(json).map_err(|err| {
        let path = err.path().to_string();
        anyhow::anyhow!("{path}: {}", err.into_inner())
    })
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
