use super::*;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};

const KEY: &str = "https://backend.test";

#[test]
fn parse_active_reads_the_flag_and_refuses_anything_else() {
    assert_eq!(
        parse_active(&json!({"active": true, "until": "x"})),
        Ok(true)
    );
    assert_eq!(
        parse_active(&json!({"active": false, "until": null})),
        Ok(false)
    );
    assert!(parse_active(&json!({"active": "yes"})).is_err());
    assert!(parse_active(&json!({})).is_err());
    assert_eq!(
        parse_active(&json!({"success": true, "data": {"active": true}})),
        Ok(true),
        "an answer still in its envelope reads the same"
    );
    assert!(parse_active(&json!({"success": true, "data": {}})).is_err());
}

#[tokio::test]
async fn an_answer_is_reused_within_the_ttl_and_asked_again_after() {
    let cache = Answer::new();
    let base = Instant::now();
    let calls = AtomicUsize::new(0);
    let ask = |active: bool| {
        calls.fetch_add(1, Ordering::SeqCst);
        async move { Ok(active) }
    };
    assert!(active_with_cache(&cache, KEY, CACHE_TTL, || base, || ask(true)).await);
    assert!(
        active_with_cache(
            &cache,
            KEY,
            CACHE_TTL,
            || base + Duration::from_secs(30),
            || ask(false)
        )
        .await,
        "still the cached answer"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        !active_with_cache(
            &cache,
            KEY,
            CACHE_TTL,
            || base + Duration::from_secs(61),
            || ask(false)
        )
        .await,
        "asked again once stale"
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn an_error_is_not_free_and_is_not_asked_again_at_once() {
    let cache = Answer::new();
    let base = Instant::now();
    let calls = AtomicUsize::new(0);
    let failing = || {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Err::<bool, _>("GET /memory/free-period failed (404 Not Found)".to_string()) }
    };
    assert!(!active_with_cache(&cache, KEY, CACHE_TTL, || base, failing).await);
    assert!(!active_with_cache(&cache, KEY, CACHE_TTL, || base, failing).await);
    assert_eq!(calls.load(Ordering::SeqCst), 1, "a failure is cached too");
}

#[tokio::test]
async fn each_backend_keeps_its_own_answer() {
    let cache = Answer::new();
    let base = Instant::now();
    let calls = AtomicUsize::new(0);
    let ask = |active: bool| {
        calls.fetch_add(1, Ordering::SeqCst);
        async move { Ok(active) }
    };
    assert!(active_with_cache(&cache, "https://a.test", CACHE_TTL, || base, || ask(true)).await);
    assert!(!active_with_cache(&cache, "https://b.test", CACHE_TTL, || base, || ask(false)).await);
    assert!(active_with_cache(&cache, "https://a.test", CACHE_TTL, || base, || ask(false)).await);
    assert_eq!(calls.load(Ordering::SeqCst), 2, "a's answer survived b's");
}

#[tokio::test]
async fn concurrent_callers_share_one_request() {
    let cache = Answer::new();
    let base = Instant::now();
    let calls = AtomicUsize::new(0);
    let ask = || {
        calls.fetch_add(1, Ordering::SeqCst);
        async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            Ok(true)
        }
    };
    let (a, b) = tokio::join!(
        active_with_cache(&cache, KEY, CACHE_TTL, || base, ask),
        active_with_cache(&cache, KEY, CACHE_TTL, || base, ask),
    );
    assert!(a && b);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn an_answer_for_one_backend_is_not_reused_for_another() {
    let cache = Answer::new();
    let base = Instant::now();
    assert!(active_with_cache(&cache, KEY, CACHE_TTL, || base, || async { Ok(true) }).await);
    assert!(
        !active_with_cache(
            &cache,
            "https://other.test",
            CACHE_TTL,
            || base,
            || async { Ok(false) }
        )
        .await
    );
}

#[tokio::test]
async fn memory_off_is_not_free() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = crate::memory::test_fixtures::config_in(&tmp);
    config.memory.engine = String::new();
    assert!(!free_period_active(&config).await);
}

#[tokio::test]
async fn an_engine_other_than_the_hosted_one_is_always_free() {
    let tmp = tempfile::tempdir().unwrap();
    let config = crate::memory::test_fixtures::config_in(&tmp);
    crate::memory::test_fixtures::bind_reference(&config);
    assert!(free_period_active(&config).await);
}

fn credential(config: &Config) -> BackendCredential {
    crate::security::credentials::session_support::resolve_backend_credential(config).unwrap()
}

fn keyed_config(tmp: &tempfile::TempDir) -> Config {
    let mut config = crate::memory::test_fixtures::config_in(tmp);
    config.secrets.encrypt = false;
    // A key of its own: the answer cache is process-wide and keyed by
    // backend and credential, and a mock server may reuse a port another
    // test's server held a moment ago.
    let key = format!("th_test_{}", tmp.path().display()).replace(['/', '.'], "_");
    crate::security::credentials::api_key::store_api_key(&config, &key).unwrap();
    config
}

async fn backend_answering(status: u16, body: serde_json::Value) -> wiremock::MockServer {
    use wiremock::matchers::{method, path};
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(method("GET"))
        .and(path("/memory/free-period"))
        .respond_with(wiremock::ResponseTemplate::new(status).set_body_json(body))
        .mount(&server)
        .await;
    server
}

#[tokio::test]
async fn the_backend_says_the_period_is_on() {
    let tmp = tempfile::tempdir().unwrap();
    let config = keyed_config(&tmp);
    let server = backend_answering(
        200,
        json!({"success": true, "data": {"active": true, "until": "2026-11-06T00:00:00.000Z"}}),
    )
    .await;
    assert_eq!(fetch(&credential(&config), &server.uri()).await, Ok(true));
}

#[tokio::test]
async fn the_backend_says_the_period_is_off() {
    let tmp = tempfile::tempdir().unwrap();
    let config = keyed_config(&tmp);
    let server = backend_answering(
        200,
        json!({"success": true, "data": {"active": false, "until": null}}),
    )
    .await;
    assert_eq!(fetch(&credential(&config), &server.uri()).await, Ok(false));
}

#[tokio::test]
async fn a_backend_without_the_route_is_an_error_read_as_not_free() {
    let tmp = tempfile::tempdir().unwrap();
    let config = keyed_config(&tmp);
    let server = backend_answering(404, json!({"success": false, "error": "Not Found"})).await;
    assert!(fetch(&credential(&config), &server.uri()).await.is_err());
    let cache = Answer::new();
    let uri = server.uri();
    let credential = credential(&config);
    assert!(
        !active_with_cache(&cache, &uri, CACHE_TTL, Instant::now, || fetch(
            &credential,
            &uri
        ))
        .await
    );
}

#[tokio::test]
async fn the_hosted_engine_asks_the_backend() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = keyed_config(&tmp);
    let server = backend_answering(
        200,
        json!({"success": true, "data": {"active": true, "until": "2026-11-06T00:00:00.000Z"}}),
    )
    .await;
    config.api_url = Some(server.uri());
    config.memory.engine = TINYHUMANS_ENGINE.to_string();
    assert!(free_period_active(&config).await);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn the_engines_own_backend_is_asked_not_the_apps() {
    let tmp = tempfile::tempdir().unwrap();
    let mut config = keyed_config(&tmp);
    let server = backend_answering(
        200,
        json!({"success": true, "data": {"active": true, "until": null}}),
    )
    .await;
    config.api_url = Some("http://127.0.0.1:9".into());
    config.memory.engine = TINYHUMANS_ENGINE.to_string();
    config.memory.engines.insert(
        TINYHUMANS_ENGINE.to_string(),
        crate::config::schema::MemoryEngineSettings {
            endpoint: Some(server.uri()),
        },
    );
    assert!(free_period_active(&config).await);
    assert_eq!(server.received_requests().await.unwrap().len(), 1);
}

#[tokio::test]
async fn each_account_keeps_its_own_answer() {
    let server = backend_answering(
        200,
        json!({"success": true, "data": {"active": true, "until": null}}),
    )
    .await;
    for key in ["th_account_a", "th_account_b"] {
        let tmp = tempfile::tempdir().unwrap();
        let mut config = crate::memory::test_fixtures::config_in(&tmp);
        config.secrets.encrypt = false;
        crate::security::credentials::api_key::store_api_key(&config, key).unwrap();
        config.api_url = Some(server.uri());
        config.memory.engine = TINYHUMANS_ENGINE.to_string();
        assert!(free_period_active(&config).await);
    }
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        2,
        "the second account asked for its own answer"
    );
}

#[test]
fn the_cache_key_digest_is_the_full_sha256() {
    let key = digest("th_account_a");
    assert_eq!(key.len(), 64, "{key}");
    assert_ne!(key, digest("th_account_b"));
}
