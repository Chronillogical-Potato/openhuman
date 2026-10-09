use super::*;

// #6932: an offline local profile is a valid sign-in, but it cannot
// authenticate managed inference.
#[test]
fn the_offline_local_session_cannot_authenticate_managed_inference() {
    use crate::security::credentials::session_support::{
        SessionTokenCheck, LOCAL_SESSION_MANAGED_INFERENCE_UNAVAILABLE,
    };

    let error = managed_bearer(SessionTokenCheck::Live("header.payload.local".to_string()))
        .expect_err("a local session has no managed bearer");

    assert_eq!(
        error.to_string(),
        LOCAL_SESSION_MANAGED_INFERENCE_UNAVAILABLE
    );
}

#[test]
fn the_refusal_does_not_read_as_an_expired_session() {
    use crate::security::credentials::session_support::SessionTokenCheck;

    let error = managed_bearer(SessionTokenCheck::Live("header.payload.local".to_string()))
        .expect_err("a local session has no managed bearer");

    assert!(!crate::core::observability::is_session_expired_message(
        &error.to_string()
    ));
}

#[test]
fn a_signed_in_session_still_authenticates_managed_inference() {
    use crate::security::credentials::session_support::SessionTokenCheck;

    let bearer = managed_bearer(SessionTokenCheck::Live(
        "header.payload.signature".to_string(),
    ))
    .expect("a hosted session is the bearer");

    assert_eq!(bearer, "header.payload.signature");
}
