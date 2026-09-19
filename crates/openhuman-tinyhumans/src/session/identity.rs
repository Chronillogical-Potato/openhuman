//! Process-global, synchronous peek at the signed-in user id.
//!
//! Sentry `before_send` hooks and similar sync callers cannot await the
//! manager, so the manager mirrors the current user id into this slot on every
//! state change. Only the id is kept — never a token, never the profile.

#[cfg(not(test))]
use std::sync::OnceLock;
use std::sync::RwLock;

#[cfg(not(test))]
fn slot() -> &'static RwLock<Option<String>> {
    static USER_ID: OnceLock<RwLock<Option<String>>> = OnceLock::new();
    USER_ID.get_or_init(|| RwLock::new(None))
}

// One slot per thread under test. Every `#[tokio::test]` here runs on its own
// current-thread runtime, so this isolates each test (and the tasks it spawns)
// from every other test's login/logout without a lock anyone must remember. A
// shared slot let a parallel login land between `logout()` and its assertion.
// A `multi_thread` test that writes from a worker thread would not see its own
// write here — keep manager tests on the default flavour.
#[cfg(test)]
fn slot() -> &'static RwLock<Option<String>> {
    thread_local! {
        static USER_ID: &'static RwLock<Option<String>> = Box::leak(Box::new(RwLock::new(None)));
    }
    USER_ID.with(|slot| *slot)
}

/// Record the signed-in user id (or clear it with `None`).
pub fn set_user_id(user_id: Option<String>) {
    let mut guard = slot().write().unwrap_or_else(|p| p.into_inner());
    *guard = user_id
        .map(|id| id.trim().to_string())
        .filter(|id| !id.is_empty());
}

/// Forget the signed-in user id.
pub fn clear() {
    set_user_id(None);
}

/// The signed-in user id, if any.
pub fn peek_user_id() -> Option<String> {
    slot().read().unwrap_or_else(|p| p.into_inner()).clone()
}
