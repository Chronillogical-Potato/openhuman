use super::*;
use crate::client::ClientHeaders;
use crate::test_support::{
    me_user, Backend, FakeCore, MeAnswer, EXPIRED_JWT, LIVE_JWT, LIVE_JWT_NO_SUB, LOCAL_TOKEN,
    OPAQUE_TOKEN,
};
use std::sync::Arc;

fn manager(core: &Arc<FakeCore>) -> Arc<SessionManager<FakeCore>> {
    SessionManager::new(Arc::clone(core), ClientHeaders::new("openhuman"))
}

async fn drain(rx: &mut tokio::sync::broadcast::Receiver<SessionEvent>) -> Vec<SessionEvent> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        out.push(event);
    }
    out
}

#[tokio::test]
async fn login_with_token_exchanges_validates_and_stores() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user())]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    let mut rx = m.subscribe();

    let state = m.login_with_token("tok").await.unwrap();
    assert!(state.core.is_authenticated);
    assert_eq!(state.core.user_id.as_deref(), Some("user-123"));
    assert_eq!(state.current_user.as_ref().unwrap()["email"], "u@example.com");
    assert!(!state.current_user_stale);

    let stored = core.session().unwrap();
    assert_eq!(stored.kind, "session");
    assert_eq!(stored.token, LIVE_JWT);
    assert_eq!(stored.user_id.as_deref(), Some("user-123"));
    assert_eq!(stored.user.unwrap()["_id"], "user-123");
    assert_eq!(identity::peek_user_id().as_deref(), Some("user-123"));
    assert!(matches!(drain(&mut rx).await.as_slice(), [SessionEvent::Changed(_)]));
}

#[tokio::test]
async fn rejected_jwt_is_never_stored() {
    let backend = Backend::start(vec![MeAnswer::Status(401)]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    assert!(matches!(
        m.store_session_token(LIVE_JWT, None).await,
        Err(SessionError::Rejected(_))
    ));
    assert_eq!(core.session(), None);
    assert!(!core.methods().iter().any(|m| m == link::AUTH_SET_CREDENTIAL));
}

#[tokio::test]
async fn expired_jwt_is_refused_locally_without_a_request() {
    let backend = Backend::start(vec![]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    assert_eq!(
        m.store_session_token(EXPIRED_JWT, None).await.unwrap_err(),
        SessionError::Expired
    );
    assert_eq!(backend.me_calls(), 0);
    assert_eq!(core.session(), None);
}

#[tokio::test]
async fn unreachable_backend_stores_a_pending_session_when_the_jwt_can_be_trusted() {
    let core = FakeCore::new("http://127.0.0.1:9");
    let m = manager(&core);
    let state = m.store_session_token(LIVE_JWT, None).await.unwrap();
    assert!(state.core.is_authenticated);
    let stored = core.session().unwrap();
    assert_eq!(stored.user_id.as_deref(), Some("user-123"));
    assert_eq!(stored.user.unwrap()[PENDING_BACKEND_VALIDATION_FIELD], true);
    // The current user is served from the stored payload, flagged stale.
    assert!(state.current_user_stale);
    m.cancel_revalidation();
}

#[tokio::test]
async fn unreachable_backend_refuses_tokens_without_exp_or_subject() {
    let core = FakeCore::new("http://127.0.0.1:9");
    let m = manager(&core);
    assert!(matches!(
        m.store_session_token(OPAQUE_TOKEN, None).await,
        Err(SessionError::Transient(_))
    ));
    assert_eq!(
        m.store_session_token(LIVE_JWT_NO_SUB, None).await.unwrap_err(),
        SessionError::UserIdUnavailable
    );
    assert_eq!(core.session(), None);
}

#[tokio::test]
async fn caller_supplied_user_id_rescues_a_subjectless_jwt_offline() {
    let core = FakeCore::new("http://127.0.0.1:9");
    let m = manager(&core);
    let user = serde_json::json!({ "id": "from-caller" });
    m.store_session_token(LIVE_JWT_NO_SUB, Some(user)).await.unwrap();
    assert_eq!(core.session().unwrap().user_id.as_deref(), Some("from-caller"));
    m.cancel_revalidation();
}

#[tokio::test]
async fn local_session_is_stored_without_touching_the_backend() {
    let backend = Backend::start(vec![]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    assert!(matches!(
        m.store_session_token(LOCAL_TOKEN, None).await,
        Err(SessionError::Invalid(_))
    ));
    let state = m
        .store_session_token(LOCAL_TOKEN, Some(serde_json::json!({ "name": "Me" })))
        .await
        .unwrap();
    assert_eq!(backend.me_calls(), 0);
    assert_eq!(core.session().unwrap().kind, "local");
    assert_eq!(state.current_user.unwrap()["name"], "Me");
}

#[tokio::test]
async fn api_key_store_and_clear() {
    let core = FakeCore::new("http://127.0.0.1:9");
    let m = manager(&core);
    let state = m.store_api_key(" sk-1 ").await.unwrap();
    assert_eq!(state.core.credential.as_deref(), Some("api-key"));
    assert_eq!(core.api_key.lock().unwrap().as_deref(), Some("sk-1"));
    let state = m.clear_api_key().await.unwrap();
    assert!(!state.core.is_authenticated);
    assert!(matches!(m.store_api_key("").await, Err(SessionError::Invalid(_))));
}

#[tokio::test]
async fn logout_clears_the_session_and_identity() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user())]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    m.login_with_token("tok").await.unwrap();
    let state = m.logout().await.unwrap();
    assert!(!state.core.is_authenticated);
    assert_eq!(core.session(), None);
    assert_eq!(identity::peek_user_id(), None);
    assert_eq!(m.cache().peek(), None);
    let (method, params) = core
        .calls()
        .into_iter()
        .find(|(m, _)| m == link::AUTH_CLEAR_CREDENTIAL)
        .unwrap();
    assert_eq!(method, link::AUTH_CLEAR_CREDENTIAL);
    assert_eq!(params["kind"], "session");
}

#[tokio::test]
async fn current_user_rejection_signs_out_and_emits_expired() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user()), MeAnswer::Status(401)]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    m.login_with_token("tok").await.unwrap();
    let mut rx = m.subscribe();
    assert!(matches!(m.current_user(true).await, Err(SessionError::Rejected(_))));
    assert_eq!(core.session(), None);
    let events = drain(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, SessionEvent::Expired { source } if source == "auth/me")));
    assert!(events
        .iter()
        .any(|e| matches!(e, SessionEvent::Changed(s) if !s.core.is_authenticated)));
    let state = m.state().await.unwrap();
    assert!(!state.core.is_authenticated);
}

#[tokio::test]
async fn current_user_serves_the_stored_user_stale_while_the_backend_is_down() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user()), MeAnswer::Status(503)]).await;
    let core = FakeCore::new(&backend.url);
    let m = manager(&core);
    m.login_with_token("tok").await.unwrap();
    m.cache().forget();
    let current = m.current_user(true).await.unwrap();
    assert!(current.stale);
    assert_eq!(current.user.unwrap()["_id"], "user-123");
    assert!(core.session().is_some(), "an outage must not sign the user out");
}

#[tokio::test]
async fn current_user_confirms_a_pending_session_once_the_backend_answers() {
    let backend = Backend::start(vec![MeAnswer::Ok(me_user())]).await;
    let core = FakeCore::new(&backend.url);
    *core.session.lock().unwrap() = Some(crate::test_support::StoredCredential {
        kind: "session".into(),
        token: LIVE_JWT.into(),
        user_id: Some("user-123".into()),
        user: Some(serde_json::json!({ PENDING_BACKEND_VALIDATION_FIELD: true })),
    });
    let m = manager(&core);
    let current = m.current_user(false).await.unwrap();
    assert_eq!(current.user.as_ref().unwrap()["email"], "u@example.com");
    let stored = core.session().unwrap();
    assert!(stored.user.unwrap().get(PENDING_BACKEND_VALIDATION_FIELD).is_none());
}

#[tokio::test]
async fn state_for_a_signed_out_core_needs_no_backend() {
    let core = FakeCore::new("http://127.0.0.1:9");
    let m = manager(&core);
    let state = m.state().await.unwrap();
    assert!(!state.core.is_authenticated);
    assert_eq!(state.current_user, None);
    assert_eq!(state.core.kind(), None);
}

#[tokio::test]
async fn client_is_rebuilt_when_the_backend_url_changes() {
    let core = FakeCore::new("http://a.example");
    let m = manager(&core);
    let first = m.client().await.unwrap();
    assert_eq!(first.base_url(), "http://a.example");
    assert!(Arc::ptr_eq(&first, &m.client().await.unwrap()));
    *core.api_url.lock().unwrap() = "http://b.example/".to_string();
    let second = m.client().await.unwrap();
    assert_eq!(second.base_url(), "http://b.example");
    assert!(!Arc::ptr_eq(&first, &second));
}

#[tokio::test]
async fn core_failures_surface_as_core_errors() {
    let core = FakeCore::new("http://127.0.0.1:9");
    *core.fail_with.lock().unwrap() = Some("boom".to_string());
    let m = manager(&core);
    assert!(matches!(m.state().await, Err(SessionError::Core(_))));
    assert!(matches!(m.logout().await, Err(SessionError::Core(_))));
    assert!(matches!(
        m.store_session_token(LIVE_JWT, None).await,
        Err(SessionError::Backend(_))
    ));
}

#[test]
fn session_error_display_carries_stable_prefixes() {
    assert!(SessionError::Rejected("x".into()).to_string().starts_with("REJECTED:"));
    assert!(SessionError::Expired.to_string().starts_with("EXPIRED:"));
    assert!(SessionError::Transient("x".into()).to_string().starts_with("TRANSIENT:"));
    assert!(SessionError::UserIdUnavailable.to_string().starts_with("USER_ID_UNAVAILABLE:"));
    assert!(SessionError::ConsumeFailed("x".into()).to_string().starts_with("CONSUME_FAILED:"));
    assert!(SessionError::Core("x".into()).to_string().starts_with("CORE:"));
}
