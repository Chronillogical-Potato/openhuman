//! Whether memory work is free for the user right now.
//!
//! Background memory jobs that write a user's whole memory again (moving it
//! into a new layout, importing a local store) start on their own only when
//! that costs the user nothing; otherwise they wait for the user to start them.
//! [`free_period_active`] is the one check every such job asks:
//!
//! - **Not the hosted engine** (self-hosted CortexDB, or a host's own engine):
//!   always free here, because no TinyHumans credit is spent.
//! - **The hosted `tinyhumans` engine**: free only while the backend's memory
//!   free period is on (`GET /memory/free-period`, through the backend
//!   transport). The answer is cached for [`CACHE_TTL`] per backend.
//! - **Anything unknown** (memory off, signed out, no transport, an error, a
//!   backend without the route): not free, so nothing starts on its own.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sha2::Digest;

use crate::backend::BackendClient;
use crate::config::Config;
use crate::security::credentials::session_support::BackendCredential;

use super::engine::{self, Binding, TINYHUMANS_ENGINE};

/// How long an answer is reused. A job polls between batches; the period
/// changes on the order of days, so a minute keeps it to one request a minute.
pub const CACHE_TTL: Duration = Duration::from_secs(60);

/// The last answer for each backend. One async lock covers the lookup and the
/// fetch on a miss, so concurrent callers (two jobs polling at once) wait for
/// one request rather than each sending their own.
struct Answer {
    inner: tokio::sync::Mutex<Option<HashMap<String, (Instant, bool)>>>,
}

impl Answer {
    const fn new() -> Self {
        Self {
            inner: tokio::sync::Mutex::const_new(None),
        }
    }
}

static ANSWER: Answer = Answer::new();

/// Whether memory work is free for the user right now. See the module docs.
pub async fn free_period_active(config: &Config) -> bool {
    let bound = match engine::resolve(config) {
        Binding::On(bound) => bound,
        Binding::Off { .. } => return false,
    };
    if bound.id != TINYHUMANS_ENGINE {
        return true;
    }
    let Ok(credential) =
        crate::security::credentials::session_support::resolve_backend_credential(config)
    else {
        return false;
    };
    // The answer is the account's on the backend memory is bound to, so the
    // cache is keyed by both: one account's answer never serves another.
    let key = format!("{}|{}", bound.endpoint, digest(credential.secret()));
    active_with_cache(&ANSWER, &key, CACHE_TTL, Instant::now, || {
        fetch(&credential, &bound.endpoint)
    })
    .await
}

/// A one-way digest of a credential for a cache key: the full SHA-256, so
/// two accounts never share a key.
fn digest(secret: &str) -> String {
    sha2::Sha256::digest(secret.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Asks `backend` whether its memory free period is on for `credential`.
async fn fetch(credential: &BackendCredential, backend: &str) -> Result<bool, String> {
    let client = BackendClient::new(backend).map_err(|e| format!("{e:#}"))?;
    let data = client
        .authed_json(
            credential.clone(),
            reqwest::Method::GET,
            "/memory/free-period",
            None,
        )
        .await
        .map_err(crate::backend::flatten_authed_error)?;
    parse_active(&data)
}

/// `active` from the route's answer; anything else is an error. The
/// transport unwraps the `{success, data}` envelope, but an answer that
/// still carries it is read the same way.
fn parse_active(data: &serde_json::Value) -> Result<bool, String> {
    data.get("data")
        .unwrap_or(data)
        .get("active")
        .and_then(serde_json::Value::as_bool)
        .ok_or_else(|| "free-period answer has no boolean `active`".to_string())
}

/// The cached answer for `key`, else `fetch`'s, with a failure read as not
/// free. Failures are cached too, so a backend without the route is asked
/// once a minute, not once a batch.
async fn active_with_cache<F, Fut>(
    cache: &Answer,
    key: &str,
    ttl: Duration,
    clock: impl Fn() -> Instant,
    fetch: F,
) -> bool
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    let mut answers = cache.inner.lock().await;
    let answers = answers.get_or_insert_with(HashMap::new);
    // Read after the lock: a wait behind another backend's fetch must not
    // stretch a cached answer past its TTL.
    let now = clock();
    if let Some((_, active)) = answers
        .get(key)
        .filter(|(at, _)| now.duration_since(*at) < ttl)
    {
        return *active;
    }
    let active = match fetch().await {
        Ok(active) => active,
        Err(error) => {
            tracing::debug!(%error, "[memory:billing] free period unknown; treating as not free");
            false
        }
    };
    answers.insert(key.to_string(), (now, active));
    active
}

#[cfg(test)]
#[path = "billing_tests.rs"]
mod tests;
