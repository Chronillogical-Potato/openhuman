//! The host the app runs the migration with: the two engines from
//! `memory::engine::bind_with_root`, placement and the switch from
//! `memory::scope`, and the free period from `memory::billing`.

use async_trait::async_trait;

use super::claim::ClaimKey;
use super::copy::Engines;
use super::job::LayoutHost;
use super::map::{FlowPlacement, Placement};
use crate::config::{Config, MemoryLayoutMode};
use crate::memory::engine::{self, CORTEXDB_ENGINE};
use crate::memory::error::{MemoryError, MemoryResult};
use crate::memory::scope::{self, MemoryIdentity};

/// The app's [`LayoutHost`].
pub struct AppHost;

#[async_trait]
impl LayoutHost for AppHost {
    fn engines(&self, config: &Config) -> MemoryResult<Engines> {
        let root = signed_in_root(config)?;
        Ok(Engines {
            legacy: engine::bind_with_root(config, None)?.engine,
            tree: engine::bind_with_root(config, Some(&root))?.engine,
        })
    }

    fn placement(&self, config: &Config) -> MemoryResult<Placement> {
        // The v3 layout, whatever the setting says: the copy runs before the
        // switch.
        let mut v3 = config.clone();
        v3.memory.layout = MemoryLayoutMode::V3;
        let layout = MemoryIdentity::root().resolve(&v3).layout;
        let chat_node = scope::chat_node(&layout);
        Ok(Placement {
            layout,
            chat_node,
            flows: FlowPlacement::WithRoot,
            split_github_by_repo: config.memory.split_github_by_repo,
        })
    }

    fn is_switched(&self, config: &Config) -> bool {
        scope::layout_is_v3(config)
    }

    async fn switch(&self, config: &Config) -> MemoryResult<()> {
        // This person's own config file, loaded fresh: never the active
        // account's (someone else's after an account switch mid-move).
        scope::switch_to_v3(config).await
    }

    async fn free_now(&self, config: &Config) -> bool {
        crate::memory::billing::free_period_active(config).await
    }

    fn legacy_claim(&self, config: &Config) -> MemoryResult<Option<ClaimKey>> {
        // A self-hosted key is the operator's: every local account using it
        // shares one legacy tree. The hosted engine is one tenant per person.
        if config.memory.engine.trim() != CORTEXDB_ENGINE {
            return Ok(None);
        }
        let owner = signed_in_root(config)?;
        // `<app>/users/<id>/config.toml`: the app directory every account shares.
        let app_dir = config
            .config_path
            .parent()
            .and_then(std::path::Path::parent)
            .and_then(std::path::Path::parent)
            .ok_or_else(signed_out)?;
        let endpoint = engine::bind_with_root(config, None)?.endpoint;
        Ok(Some(ClaimKey::new(app_dir, &endpoint, &owner)))
    }
}

fn signed_in_root(config: &Config) -> MemoryResult<String> {
    scope::user_root(config).ok_or_else(signed_out)
}

fn signed_out() -> MemoryError {
    MemoryError::Off("sign in to move memory into its own layout".to_string())
}

#[cfg(test)]
#[path = "app_host_tests.rs"]
mod tests;
