//! Test-only storage for module calls while a unit test holds a module lock
//! (the connector module's `module_guard`). Kept out of the process storage
//! slot so it is never another concurrent test's storage (MAIN-57).

use std::sync::Arc;

use crate::storage::StorageBackend;

/// Storage for module calls in unit tests that hold a module lock (the
/// connector module's `module_guard`). A module call carries no workspace, so
/// such a test needs a backend; this one is read only here, so it is never
/// another test's storage the way an installed process slot is (MAIN-57).
static TEST_MODULE_STORAGE: std::sync::Mutex<Option<Arc<dyn StorageBackend>>> =
    std::sync::Mutex::new(None);

pub(super) fn test_module_storage() -> Option<Arc<dyn StorageBackend>> {
    TEST_MODULE_STORAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Serve `backend` to module calls until the returned guard drops. Hold the
/// module's test lock for as long as it lives.
pub(crate) fn serve_test_module_storage(backend: Arc<dyn StorageBackend>) -> TestModuleStorage {
    let previous = TEST_MODULE_STORAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .replace(backend);
    TestModuleStorage { previous }
}

/// Withdraw the backend module calls are served, as a storage outage would,
/// while the guard from [`serve_test_module_storage`] is still held.
pub(crate) fn withdraw_test_module_storage() {
    *TEST_MODULE_STORAGE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
}

/// Restores the previous test module storage on drop.
pub(crate) struct TestModuleStorage {
    previous: Option<Arc<dyn StorageBackend>>,
}

impl Drop for TestModuleStorage {
    fn drop(&mut self) {
        *TEST_MODULE_STORAGE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = self.previous.take();
    }
}
