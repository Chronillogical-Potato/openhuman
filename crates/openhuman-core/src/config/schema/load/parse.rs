//! The one place a `config.toml` document becomes a [`Config`].
//!
//! Deserializing the config tree straight from TOML instantiates every derived
//! `Deserialize` impl in it once per `toml` map-access type (about six), and the
//! tree has well over a hundred types. Parsing into a [`toml::Value`] first and
//! decoding that through [`serde_json::Value`] keeps one decoder instantiation
//! per type. Syntax errors still carry TOML line and column; a type error keeps
//! the serde message but not the location.

use super::Config;

/// Parse a `config.toml` document into a [`Config`].
#[inline(never)]
pub(crate) fn config_from_toml_str(contents: &str) -> anyhow::Result<Config> {
    let document: toml::Value = toml::from_str(contents)?;
    let json = serde_json::to_value(document)?;
    Ok(serde_json::from_value(json)?)
}

#[cfg(test)]
#[path = "parse_tests.rs"]
mod tests;
