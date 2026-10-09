//! Crash-reporting client ownership for the standalone terminal binary.
//!
//! The client options come from embed's shared chain
//! (`openhuman_rpc::embed::process::sentry`): the same noise filters, secret
//! scrubbing, PII-off defaults and transport as the CLI and the desktop shell.
//! Only the DSN, release and environment are resolved here.

#[cfg(feature = "crash-reporting")]
use openhuman_rpc::embed::process::{self, sentry};

/// Initialize the Sentry client before the TUI installs its tracing subscriber.
///
/// The returned guard must live for the whole process. A build without the
/// `crash-reporting` feature retains the same call site and compiles to a no-op.
#[cfg(feature = "crash-reporting")]
pub fn init_crash_reporting() -> sentry::sdk::ClientInitGuard {
    // Match the core binary: startup consumers must see a repository-local
    // `.env`, while explicit process variables continue to take precedence.
    // Failure is not fatal here; `run_from_cli` loads it again and reports it.
    let _ = process::load_dotenv_for_cli();

    let config = sentry::SentryConfig::new(
        sentry_dsn(),
        build_release_tag(),
        resolve_environment(std::env::var("OPENHUMAN_APP_ENV").ok()),
    );
    sentry::sdk::init(sentry::client_options(config))
}

#[cfg(not(feature = "crash-reporting"))]
pub fn init_crash_reporting() {}

/// `OPENHUMAN_CORE_SENTRY_DSN`, then the legacy `OPENHUMAN_SENTRY_DSN`, at
/// runtime and then baked in at compile time.
#[cfg(feature = "crash-reporting")]
fn sentry_dsn() -> Option<String> {
    sentry::first_non_blank([
        std::env::var("OPENHUMAN_CORE_SENTRY_DSN").ok(),
        std::env::var("OPENHUMAN_SENTRY_DSN").ok(),
        option_env!("OPENHUMAN_CORE_SENTRY_DSN").map(str::to_owned),
        option_env!("OPENHUMAN_SENTRY_DSN").map(str::to_owned),
    ])
}

#[cfg(feature = "crash-reporting")]
fn build_release_tag() -> String {
    sentry::release_tag(
        env!("CARGO_PKG_VERSION"),
        option_env!("OPENHUMAN_BUILD_SHA"),
    )
}

#[cfg(feature = "crash-reporting")]
fn resolve_environment(app_env: Option<String>) -> String {
    if let Some(value) = app_env {
        let value = value.trim().to_ascii_lowercase();
        if !value.is_empty() {
            return value;
        }
    }
    if cfg!(debug_assertions) {
        "development".to_string()
    } else {
        "production".to_string()
    }
}

#[cfg(all(test, feature = "crash-reporting"))]
#[path = "crash_reporting_tests.rs"]
mod tests;
