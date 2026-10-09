use super::*;

#[test]
fn every_variant_has_its_stable_code() {
    assert_eq!(MemoryError::Off("x".into()).code(), MEMORY_OFF);
    assert_eq!(MemoryError::Unsupported("x".into()).code(), UNSUPPORTED);
    assert_eq!(MemoryError::invalid("x").code(), INVALID_REQUEST);
    assert_eq!(MemoryError::Unauthorized("x".into()).code(), UNAUTHORIZED);
    assert_eq!(MemoryError::Engine("x".into()).code(), ENGINE);
    assert_eq!(
        MemoryError::InsufficientCredits("x".into()).code(),
        INSUFFICIENT_CREDITS
    );
    assert_eq!(MemoryError::Unavailable("x".into()).code(), UNAVAILABLE);
}

#[test]
fn structured_error_carries_the_code_and_flags_user_states() {
    let off = MemoryError::Off("sign in".into()).to_structured();
    assert_eq!(off.data.as_ref().unwrap()["code"], MEMORY_OFF);
    assert_eq!(off.data.as_ref().unwrap()["kind"], MEMORY_OFF);
    assert!(off.expected_user_state);
    assert!(off.message.contains("sign in"));

    let engine = MemoryError::Engine("boom".into()).to_structured();
    assert!(!engine.expected_user_state);
}

#[test]
fn tinymemory_errors_map_onto_the_taxonomy() {
    use tinymemory_api::Error as E;
    let cases = [
        (E::Unsupported("a".into()), UNSUPPORTED),
        (E::InvalidRequest("a".into()), INVALID_REQUEST),
        (E::NotFound("a".into()), INVALID_REQUEST),
        (E::Config("a".into()), INVALID_REQUEST),
        (E::Unauthorized("a".into()), UNAUTHORIZED),
        (E::Conflict("a".into()), ENGINE),
        (E::Unavailable("a".into()), UNAVAILABLE),
        (E::Engine("a".into()), ENGINE),
    ];
    for (error, code) in cases {
        assert_eq!(MemoryError::from(error).code(), code);
    }
}

#[test]
fn into_string_encodes_the_structured_error() {
    let encoded: String = MemoryError::invalid("bad").into();
    assert!(encoded.contains(INVALID_REQUEST), "{encoded}");
    assert!(encoded.contains("bad"), "{encoded}");
}

#[test]
fn account_wide_refusals_get_their_own_codes() {
    use tinymemory_api::Error as E;
    let credits = MemoryError::from(E::Engine(
        "[USER_INSUFFICIENT_CREDITS] memory API recall on h: the account has insufficient \
         credits (HTTP 402)"
            .into(),
    ));
    assert_eq!(credits.code(), INSUFFICIENT_CREDITS);
    assert!(credits.is_account_wide());
    assert!(credits.to_structured().expected_user_state);

    let down = MemoryError::from(E::Unavailable("timed out".into()));
    assert_eq!(down.code(), UNAVAILABLE);
    assert!(down.is_account_wide());

    // Another backend code on an engine failure stays an engine failure.
    let other = MemoryError::from(E::Engine("[HTTP_500] boom".into()));
    assert_eq!(other.code(), ENGINE);
    assert!(!other.is_account_wide());
    assert!(!MemoryError::invalid("bad").is_account_wide());
}

#[test]
fn a_skip_reason_reads_back_only_account_wide_refusals() {
    let code = |reason: &str| MemoryError::refusal_from_skip_reason(reason).map(|e| e.code());
    assert_eq!(
        code("engine error: [USER_INSUFFICIENT_CREDITS] memory API fetch (HTTP 402)"),
        Some(INSUFFICIENT_CREDITS)
    );
    assert_eq!(code("unavailable: connection refused"), Some(UNAVAILABLE));
    assert_eq!(code("unauthorized: key rejected"), Some(UNAUTHORIZED));
    assert_eq!(code("engine error: index corrupt"), None);
    assert_eq!(code("invalid request: bad filter"), None);
    assert_eq!(code("empty"), None);
}
