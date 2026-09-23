//! Wait out the memory module's load in a suite that boots the core in-process.
//!
//! The module is loaded once per process, by whichever caller asks first, and
//! for the whole of that load a memory call that reaches the driver answers
//! "memory is still starting ... try again in a moment". A test sharing its
//! process with earlier memory tests usually finds the module already loaded;
//! one running in its own process (cargo nextest) always asks during the load.
//! Call [`settle`] after the test's workspace env is in place and before its
//! first memory call. This is the in-crate `memory::test_support::
//! settle_memory_module` for suites outside the core.
//!
//! Include with `#[path = "support/memory_module.rs"] mod memory_module;`.

#![allow(dead_code)]

use std::time::Duration;

/// Load the memory module for the core's current config, waiting up to 60 s.
///
/// A load that fails is left for the test to observe through its own calls;
/// only a load still running after the bound panics, because every assertion
/// after it would race that load.
pub async fn settle() {
    let config = openhuman_core::config::load_config_with_timeout()
        .await
        .expect("load config to settle the memory module");
    match openhuman_core::modules::ops::ensure_loaded_within(
        &config,
        openhuman_core::memory::binding::MODULE_ID,
        Some(Duration::from_secs(60)),
    )
    .await
    {
        Ok(()) | Err(openhuman_core::modules::ops::LoadError::Failed(_)) => {}
        Err(openhuman_core::modules::ops::LoadError::StillLoading) => {
            panic!("the memory module did not finish loading within 60s")
        }
    }
}
