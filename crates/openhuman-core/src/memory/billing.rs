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

use crate::backend::BackendClient;
use crate::config::Config;

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
    let engine_id = match engine::resolve(config) {
        Binding::On(bound) => bound.id,
        Binding::Off { .. } => return false,
    };
    if engine_id != TINYHUMANS_ENGINE {
        return true;
    }
    let Ok(backend) = crate::backend::require_base_url(&config.api_url) else {
        return false;
    };
    active_with_cache(&ANSWER, &backend, CACHE_TTL, Instant::now(), || {
        fetch(config, &backend)
    })
    .await
}

/// Asks the backend whether its memory free period is on.
async fn fetch(config: &Config, backend: &str) -> Result<bool, String> {
    let credential =
        crate::security::credentials::session_support::resolve_backend_credential(config)?;
    let client = BackendClient::new(backend).map_err(|e| format!("{e:#}"))?;
    let data = client
        .authed_json(
            &credential,
            reqwest::Method::GET,
            "/memory/free-period",
            None,
        )
        .await
        .map_err(crate::backend::flatten_authed_error)?;
    parse_active(&data)
}

/// `active` from the route's answer; anything else is an error.
fn parse_active(data: &serde_json::Value) -> Result<bool, String> {
    data.get("active")
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
    now: Instant,
    fetch: F,
) -> bool
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<bool, String>>,
{
    let mut answers = cache.inner.lock().await;
    let answers = answers.get_or_insert_with(HashMap::new);
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
