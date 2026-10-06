//! The app environment (`production`, `staging`, …) the process runs in.
//!
//! Read from `OPENHUMAN_APP_ENV` / `VITE_OPENHUMAN_APP_ENV` at runtime, then
//! from the same keys baked in at compile time. The core uses it to keep a
//! staging profile's workspace apart from production; the backend host uses it
//! to pick its default base URL.

/// Runtime env key used by the Tauri/core side to select the app environment.
pub const APP_ENV_VAR: &str = "OPENHUMAN_APP_ENV";

/// Runtime env key exposed to the Vite frontend bundle. Mirrors `APP_ENV_VAR`
/// so both the core sidecar and the renderer agree on the environment without
/// a separate IPC round-trip.
pub const VITE_APP_ENV_VAR: &str = "VITE_OPENHUMAN_APP_ENV";

/// Resolve the app environment string (e.g. `"staging"`, `"production"`).
///
/// Runtime vars first, then compile-time bakes, each key checked
/// independently.
pub fn app_env_from_env() -> Option<String> {
    for key in [APP_ENV_VAR, VITE_APP_ENV_VAR] {
        if let Ok(v) = std::env::var(key) {
            let s = v.trim().to_ascii_lowercase();
            if !s.is_empty() {
                return Some(s);
            }
        }
    }

    for v in compile_time_app_env_values().into_iter().flatten() {
        let s = v.trim().to_ascii_lowercase();
        if !s.is_empty() {
            return Some(s);
        }
    }

    None
}

/// Return `true` when `app_env` equals `"staging"` (case-insensitive).
pub fn is_staging_app_env(app_env: Option<&str>) -> bool {
    matches!(app_env.map(str::trim), Some(env) if env.eq_ignore_ascii_case("staging"))
}

// ─── Compile-time env accessors ───────────────────────────────────────────────

/// Values baked in by the build pipeline. Stubbed to `[None, None]` in tests so
/// clearing the runtime vars gives deterministic results.
#[cfg(not(test))]
fn compile_time_app_env_values() -> [Option<&'static str>; 2] {
    [
        option_env!("OPENHUMAN_APP_ENV"),
        option_env!("VITE_OPENHUMAN_APP_ENV"),
    ]
}

#[cfg(test)]
fn compile_time_app_env_values() -> [Option<&'static str>; 2] {
    [None, None]
}

/// Process-global mutex serialising every test that mutates the app-env or
/// backend-URL env vars. `std::env` is process-global, so a module-local lock
/// cannot stop tests in other modules racing on the same vars.
#[cfg(test)]
static ENV_TEST_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Blocking form for synchronous tests (panics inside a tokio runtime).
#[cfg(test)]
pub(crate) fn env_test_lock() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_TEST_LOCK.blocking_lock()
}

/// Async form for `#[tokio::test]` bodies, so the guard may be held across `.await`.
#[cfg(test)]
pub(crate) async fn env_test_lock_async() -> tokio::sync::MutexGuard<'static, ()> {
    ENV_TEST_LOCK.lock().await
}

#[cfg(test)]
#[path = "app_env_tests.rs"]
mod tests;
