use super::*;
use crate::client::ClientHeaders;
use crate::test_support::{me_user, Backend, MeAnswer, LIVE_JWT};
use serde_json::json;

fn client(backend: &Backend) -> SessionClient {
    SessionClient::new(&backend.url, &ClientHeaders::new("openhuman")).unwrap()
}

#[test]
fn fetch_timeout_parsing_clamps_to_range() {
    assert_eq!(parse_fetch_timeout_secs(None), DEFAULT_FETCH_TIMEOUT_SECS);
    assert_eq!(parse_fetch_timeout_secs(Some(" 8 ")), 8);
    assert_eq!(
        parse_fetch_timeout_secs(Some("1")),
        DEFAULT_FETCH_TIMEOUT_SECS
    );
    assert_eq!(
        parse_fetch_timeout_secs(Some("13")),
        DEFAULT_FETCH_TIMEOUT_SECS
    );
    assert_eq!(
        parse_fetch_timeout_secs(Some("abc")),
        DEFAULT_FETCH_TIMEOUT_SECS
    );
}

#[test]
fn backoff_doubles_from_a_floor_and_saturates() {
    let t = Duration::from_secs(5);
    assert_eq!(backoff_base_for(t), Duration::from_secs(10));
    assert_eq!(
        backoff_base_for(Duration::from_secs(12)),
        Duration::from_secs(24)
    );
    assert_eq!(backoff_for(0, t), Duration::from_secs(10));
    assert_eq!(backoff_for(1, t), Duration::from_secs(10));
    assert_eq!(backoff_for(2, t), Duration::from_secs(20));
    assert_eq!(backoff_for(3, t), Duration::from_secs(40));
    assert_eq!(backoff_for(4, t), BACKOFF_MAX);
    assert_eq!(backoff_for(40, t), BACKOFF_MAX);
}

#[tokio::test]
async fn fresh_entry_is_served_without_a_request() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user())]).await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    let first = cache
        .get_or_refresh(&client(&backend), &cred, false)
        .await
        .unwrap();
    assert_eq!(first.user.as_ref().unwrap()["_id"], "user-123");
    assert!(!first.stale);
    assert_eq!(first.stale_seconds, Some(0));
    let second = cache
        .get_or_refresh(&client(&backend), &cred, false)
        .await
        .unwrap();
    assert_eq!(second.user, first.user);
    assert_eq!(backend.me_calls(), 1);
    assert_eq!(cache.peek(), first.user);
}

#[tokio::test]
async fn force_bypasses_the_cache() {
    let backend = Backend::start(vec![
        MeAnswer::Ok(me_user()),
        MeAnswer::Ok(json!({ "_id": "user-123", "name": "Renamed" })),
    ])
    .await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    cache
        .get_or_refresh(&client(&backend), &cred, false)
        .await
        .unwrap();
    let forced = cache
        .get_or_refresh(&client(&backend), &cred, true)
        .await
        .unwrap();
    assert_eq!(forced.user.unwrap()["name"], "Renamed");
    assert_eq!(backend.me_calls(), 2);
}

#[tokio::test]
async fn concurrent_cache_misses_share_one_refresh() {
    let backend = Backend::start(vec![MeAnswer::Slow(50)]).await;
    let cache = CurrentUserCache::new();
    let credential = Credential::session(LIVE_JWT.as_str());
    let client = client(&backend);

    let (first, second) = tokio::join!(
        cache.get_or_refresh(&client, &credential, false),
        cache.get_or_refresh(&client, &credential, false),
    );

    assert_eq!(first.unwrap().user, second.unwrap().user);
    assert_eq!(
        backend.me_calls(),
        1,
        "cache misses must share one /auth/me request"
    );
}

// #6318 — a caller that queues behind a cold-cache refresh must observe the
// backoff window that refresh's *failure* just opened, not repeat the same
// doomed request. Before the fix, a waiter rechecked only the positive cache
// after acquiring `inflight`, so a failed first refresh sent every waiter to
// the network again.
#[tokio::test]
async fn a_failed_first_refresh_suppresses_the_waiter_queued_behind_it() {
    let backend = Backend::start(vec![MeAnswer::SlowStatus(50, 503), MeAnswer::Status(503)]).await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    let c = client(&backend);

    let (first, second) = tokio::join!(
        cache.get_or_refresh(&c, &cred, false),
        cache.get_or_refresh(&c, &cred, false),
    );

    // Whichever caller lost the race to own the refresh must see the outage
    // too (never a silent success), but must not have paid for its own
    // request — the point of coalescing behind `inflight` at all.
    for outcome in [first, second] {
        assert!(
            matches!(
                outcome,
                Err(FetchMeError::Transient(_)) | Err(FetchMeError::Suppressed { .. })
            ),
            "expected an outage error, got {outcome:?}"
        );
    }
    assert_eq!(
        backend.me_calls(),
        1,
        "the caller queued behind the failed refresh must not issue a second request"
    );
}

#[tokio::test]
async fn availability_failure_opens_a_backoff_window() {
    let backend = Backend::start(vec![MeAnswer::Status(503)]).await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    let c = client(&backend);
    assert!(matches!(
        cache.get_or_refresh(&c, &cred, false).await,
        Err(FetchMeError::Transient(_))
    ));
    match cache.get_or_refresh(&c, &cred, false).await {
        Err(FetchMeError::Suppressed { consecutive, .. }) => assert_eq!(consecutive, 1),
        other => panic!("expected suppressed replay, got {other:?}"),
    }
    assert_eq!(backend.me_calls(), 1, "the replay must not hit the network");

    // A different identity is not suppressed by this one's outage.
    let other = Credential::session("other-token");
    assert!(matches!(
        cache.get_or_refresh(&c, &other, false).await,
        Err(FetchMeError::Transient(_))
    ));
    assert_eq!(backend.me_calls(), 2);

    // `force` ignores the window, and `forget` clears it.
    assert!(matches!(
        cache.get_or_refresh(&c, &cred, true).await,
        Err(FetchMeError::Transient(_))
    ));
    cache.forget();
    assert!(matches!(
        cache.get_or_refresh(&c, &cred, false).await,
        Err(FetchMeError::Transient(_))
    ));
    assert_eq!(backend.me_calls(), 4);
}

#[tokio::test]
async fn rejection_is_retained_until_the_owner_observes_it() {
    let backend = Backend::start(vec![MeAnswer::Status(401)]).await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    let c = client(&backend);
    for _ in 0..2 {
        assert!(matches!(
            cache.get_or_refresh(&c, &cred, false).await,
            Err(FetchMeError::Rejected(_))
        ));
    }
    assert_eq!(backend.me_calls(), 1);
}

#[tokio::test]
async fn success_clears_the_failure_and_stamps_freshness() {
    let backend = Backend::start(vec![MeAnswer::Status(503), MeAnswer::Ok(me_user())]).await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    let c = client(&backend);
    let _ = cache.get_or_refresh(&c, &cred, false).await;
    let ok = cache.get_or_refresh(&c, &cred, true).await.unwrap();
    assert!(!ok.stale);
    assert_eq!(ok.stale_seconds, Some(0));
    let cached = cache.get_or_refresh(&c, &cred, false).await.unwrap();
    assert!(!cached.stale);
    assert_eq!(backend.me_calls(), 2);
}

#[tokio::test]
async fn forget_drops_the_positive_entry() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user())]).await;
    let cache = CurrentUserCache::new();
    let cred = Credential::session(LIVE_JWT.as_str());
    cache
        .get_or_refresh(&client(&backend), &cred, false)
        .await
        .unwrap();
    cache.forget();
    assert_eq!(cache.peek(), None);
    cache
        .get_or_refresh(&client(&backend), &cred, false)
        .await
        .unwrap();
    assert_eq!(backend.me_calls(), 2);
}
