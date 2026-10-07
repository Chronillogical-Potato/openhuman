//! Spawning tasks without dropping who they work for.
//!
//! The ambient [`CoreContext`] and the memory identity are tokio task-locals,
//! so a bare `tokio::spawn` starts its task with neither: the child falls back
//! to the process default context and the root memory identity. In a
//! single-user process that default is the right user by accident. In a SaaS
//! process it is the operator, so a dropped scope writes one user's work
//! somewhere it does not belong — or, with the SaaS config redirect, fails.
//!
//! [`spawn_scoped`] captures both at the call site and re-enters them in the
//! child. New code in the core spawns through it; `scripts/ci/check-saas-ambient.mjs`
//! ratchets the bare spawns that remain.

use std::future::Future;

use tokio::task::JoinHandle;

use super::CoreContext;

/// Like `tokio::spawn`, but the task keeps the caller's [`CoreContext`] and
/// memory identity.
pub fn spawn_scoped<F>(fut: F) -> JoinHandle<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(scoped(fut))
}

/// `fut`, wrapped to run under the caller's context and memory identity
/// wherever it is eventually polled.
pub fn scoped<F>(fut: F) -> impl Future<Output = F::Output> + Send
where
    F: Future + Send,
    F::Output: Send,
{
    let identity = crate::memory::scope::current();
    let fut = CoreContext::propagate(fut);
    async move {
        match identity {
            Some(identity) => crate::memory::scope::within(identity, fut).await,
            None => fut.await,
        }
    }
}

/// Like `tokio::task::spawn_blocking`, but the closure runs under the caller's
/// [`CoreContext`].
pub fn spawn_blocking_scoped<F, R>(f: F) -> JoinHandle<R>
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    let ctx = CoreContext::current();
    let handle = tokio::runtime::Handle::current();
    tokio::task::spawn_blocking(move || match ctx {
        Some(ctx) => handle.block_on(CoreContext::scope(ctx, async move { f() })),
        None => f(),
    })
}

#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
