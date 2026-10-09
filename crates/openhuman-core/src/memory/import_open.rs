//! Opening the legacy store for the import.

use std::path::Path;

use tinymemory_integrations::import::{Error, LegacyWorkspace};

/// Opens the legacy store for every scan, count, import and retry, skipping
/// connector (Composio) syncs so the scan counts match the run.
pub(super) fn open_legacy(dir: &Path) -> Result<LegacyWorkspace, Error> {
    tracing::debug!(workspace = %dir.display(), "[memory:import] skipping connector syncs");
    LegacyWorkspace::open(dir).map(|workspace| workspace.skip_connector_syncs(true))
}
