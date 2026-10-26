//! Host-side workspace registry for the module-owned audit stream.

use crate::config::AuditConfig;
use anyhow::Result;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Host handle retaining the audit's enabled setting for tool assembly.
/// Event storage and ordering belong to TinySecurity and its `AuditSink2`.
pub struct AuditLogger {
    enabled: bool,
}

static WORKSPACE_AUDIT_LOGGERS: OnceLock<Mutex<HashMap<PathBuf, Arc<AuditLogger>>>> =
    OnceLock::new();

pub fn get_or_create_workspace_audit_logger(
    config: AuditConfig,
    openhuman_dir: PathBuf,
) -> Result<Arc<AuditLogger>> {
    let workspace = std::fs::canonicalize(&openhuman_dir).unwrap_or(openhuman_dir);
    let registry = WORKSPACE_AUDIT_LOGGERS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = registry.lock();
    if let Some(existing) = map.get(&workspace) {
        return Ok(Arc::clone(existing));
    }
    let logger = Arc::new(AuditLogger::new(config, workspace.clone())?);
    map.insert(workspace, Arc::clone(&logger));
    Ok(logger)
}

impl AuditLogger {
    pub fn disabled() -> Arc<Self> {
        Arc::new(Self { enabled: false })
    }

    pub fn new(config: AuditConfig, _workspace: PathBuf) -> Result<Self> {
        Ok(Self {
            enabled: config.enabled,
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
