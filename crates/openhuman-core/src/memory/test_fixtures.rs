//! Shared fixtures for the memory unit tests: a config rooted in a temp dir,
//! and TinyMemory's in-memory reference engine bound to it.

use std::sync::Arc;

use tinymemory_api::conformance::ReferenceEngine;
use tinymemory_api::{ListRequest, MemoryEngine, MetaFilter};

use crate::config::Config;

/// A config whose workspace, action dir and credential store live in `tmp`.
/// The engine stays `tinyhumans` with no credential, so memory is off until a
/// test binds an engine.
pub(crate) fn config_in(tmp: &tempfile::TempDir) -> Config {
    let workspace = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace).expect("workspace dir");
    Config {
        workspace_dir: workspace.clone(),
        action_dir: workspace,
        config_path: tmp.path().join("config.toml"),
        ..Config::default()
    }
}

/// Binds a fresh reference engine to `config`'s workspace and returns it.
pub(crate) fn bind_reference(config: &Config) -> Arc<ReferenceEngine> {
    let engine = Arc::new(ReferenceEngine::new());
    crate::memory::engine::install_test_engine(&config.workspace_dir, engine.clone());
    engine
}

/// Every item `engine` holds that matches `filter`.
pub(crate) async fn stored(
    engine: &ReferenceEngine,
    filter: MetaFilter,
) -> Vec<tinymemory_api::Hit> {
    engine
        .list(ListRequest {
            filter,
            limit: 100,
            cursor: None,
        })
        .await
        .expect("list")
        .items
}

/// An engine that refuses every operation with one error, the way the hosted
/// engine refuses a whole account (no credits, a rejected key, an outage).
/// It describes itself as the reference engine.
pub(crate) struct RefusingEngine {
    inner: ReferenceEngine,
    error: tinymemory_api::Error,
}

impl RefusingEngine {
    /// The hosted engine's refusal for an exhausted credit balance (HTTP 402).
    pub(crate) fn out_of_credits() -> Self {
        Self::with(tinymemory_api::Error::Engine(
            "[USER_INSUFFICIENT_CREDITS] memory API fetch on api.example: the account has \
             insufficient credits (HTTP 402)"
                .into(),
        ))
    }

    /// Refuses every operation with `error`.
    pub(crate) fn with(error: tinymemory_api::Error) -> Self {
        Self {
            inner: ReferenceEngine::new(),
            error,
        }
    }

    /// Binds a refusing engine to `config`'s workspace.
    pub(crate) fn bind(self, config: &Config) {
        crate::memory::engine::install_test_engine(&config.workspace_dir, Arc::new(self));
    }
}

#[async_trait::async_trait]
impl MemoryEngine for RefusingEngine {
    fn descriptor(&self) -> &tinymemory_api::EngineDescriptor {
        self.inner.descriptor()
    }

    async fn health(&self) -> tinymemory_api::EngineHealth {
        self.inner.health().await
    }

    async fn recall(
        &self,
        _req: tinymemory_api::RecallRequest,
    ) -> tinymemory_api::Result<tinymemory_api::RecallAnswer> {
        Err(self.error.clone())
    }

    async fn fetch(
        &self,
        _req: tinymemory_api::FetchRequest,
    ) -> tinymemory_api::Result<tinymemory_api::FetchPage> {
        Err(self.error.clone())
    }

    async fn store(
        &self,
        _item: tinymemory_api::StoreItem,
    ) -> tinymemory_api::Result<tinymemory_api::StoreReceipt> {
        Err(self.error.clone())
    }

    async fn forget(
        &self,
        _target: tinymemory_api::ForgetTarget,
    ) -> tinymemory_api::Result<tinymemory_api::ForgetReport> {
        Err(self.error.clone())
    }

    async fn list(&self, _req: ListRequest) -> tinymemory_api::Result<tinymemory_api::ListPage> {
        Err(self.error.clone())
    }

    async fn export(
        &self,
        _req: ListRequest,
    ) -> tinymemory_api::Result<tinymemory_api::ExportPage> {
        Err(self.error.clone())
    }

    async fn consolidate(
        &self,
        _req: tinymemory_api::ConsolidateRequest,
    ) -> tinymemory_api::Result<tinymemory_api::ConsolidateReceipt> {
        Err(self.error.clone())
    }
}
