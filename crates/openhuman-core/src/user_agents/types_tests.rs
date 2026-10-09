use super::*;

#[test]
fn a_user_maps_to_one_stable_agent() {
    let a = UserAgentId::for_user("user-123").unwrap();
    assert_eq!(a, UserAgentId::for_user("user-123").unwrap());
    assert_ne!(a, UserAgentId::for_user("user-124").unwrap());
}

#[test]
fn the_id_fits_the_agent_charset_and_hides_the_user_id() {
    let id = UserAgentId::for_user("alice@example.com").unwrap();
    let raw = id.as_str();
    assert!(raw.len() <= 64);
    assert!(raw
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_'));
    assert!(!raw.contains("alice"));
}

#[test]
fn bad_user_ids_are_refused() {
    assert!(UserAgentId::for_user("").is_err());
    assert!(UserAgentId::for_user(&"x".repeat(MAX_USER_ID_LEN + 1)).is_err());
    assert!(UserAgentId::for_user("a\nb").is_err());
    assert!(UserAgentId::for_user(&"x".repeat(MAX_USER_ID_LEN)).is_ok());
}

#[test]
fn only_derived_ids_parse() {
    let id = UserAgentId::for_user("u").unwrap();
    assert_eq!(UserAgentId::parse(id.as_str()).unwrap(), id);
    for bad in [
        "orchestrator",
        "u-",
        "u-XYZ",
        "u-../../etc",
        "u-0123456789abcdef0123456789abcdeg",
        "u-0123456789abcdef0123456789abcdef0",
    ] {
        assert!(UserAgentId::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn serde_round_trips_and_validates() {
    let id = UserAgentId::for_user("u").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(serde_json::from_str::<UserAgentId>(&json).unwrap(), id);
    assert!(serde_json::from_str::<UserAgentId>("\"orchestrator\"").is_err());
}
