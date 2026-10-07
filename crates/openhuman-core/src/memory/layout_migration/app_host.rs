//! The host the app runs the migration with: the two engines from
//! `memory::engine::bind_with_root`, placement and the switch from
//! `memory::scope`, and the free period from `memory::billing`.

use async_trait::async_trait;

use super::copy::Engines;
use super::host::LayoutHost;
use super::map::{FlowPlacement, Placement};
use crate::config::Config;
use crate::memory::engine::{self, CORTEXDB_ENGINE};
use crate::memory::error::{MemoryError, MemoryResult};
use crate::memory::scope::{self, MemoryIdentity};

/// The app's [`LayoutHost`].
pub struct AppHost;

#[async_trait]
impl LayoutHost for AppHost {
    fn engines(&self, config: &Config) -> MemoryResult<Engines> {
        let root = scope::user_root(config).ok_or_else(|| {
            MemoryError::Off("sign in to move memory into its own layout".to_string())
        })?;
        Ok(Engines {
            legacy: engine::bind_with_root(config, None)?.engine,
            tree: engine::bind_with_root(config, Some(&root))?.engine,
        })
    }

    fn placement(&self, config: &Config) -> MemoryResult<Placement> {
        let layout = MemoryIdentity::root().resolve(config).layout;
        let chat_node = scope::chat_node(&layout);
        Ok(Placement {
            layout,
            chat_node,
            flows: FlowPlacement::WithRoot,
        })
    }

    fn is_switched(&self, config: &Config) -> bool {
        scope::layout_is_v3(config)
    }

    async fn switch(&self, _config: &Config) -> MemoryResult<()> {
        scope::switch_to_v3().await
    }

    async fn free_now(&self, config: &Config) -> bool {
        crate::memory::billing::free_period_active(config).await
    }

    fn shared_legacy(&self, config: &Config) -> bool {
        // A self-hosted key is the operator's: every local account using it
        // shares one legacy tree. The hosted engine is one tenant per person.
        config.memory.engine.trim() == CORTEXDB_ENGINE
    }
}

#[cfg(test)]
#[path = "app_host_tests.rs"]
mod tests;
