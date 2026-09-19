use super::*;

#[test]
fn build_api_path_encodes_reserved_characters_in_segments() {
    let path = build_api_path(&["teams", "team/with?reserved", "members", "user#frag"])
        .expect("path should build");

    assert_eq!(path, "/teams/team%2Fwith%3Freserved/members/user%23frag");
}

#[test]
fn build_api_path_empty_segments_list_is_root() {
    let path = build_api_path(&[]).expect("path should build");
    assert_eq!(path, "/");
}

#[test]
fn build_api_path_preserves_segment_order() {
    let path = build_api_path(&["a", "b", "c"]).expect("path should build");
    assert_eq!(path, "/a/b/c");
}

#[test]
fn build_api_path_percent_encodes_spaces_and_unicode() {
    let path = build_api_path(&["teams", "with space", "👥"]).expect("path should build");
    assert!(path.contains("with%20space"));
    // Unicode must be percent-encoded (UTF-8 bytes).
    assert!(!path.contains('👥'));
}

#[test]
fn normalize_id_rejects_empty_with_field_name() {
    let err = normalize_id("", "teamId").unwrap_err();
    assert_eq!(err, "teamId is required");
}

#[test]
fn normalize_id_rejects_whitespace_only() {
    let err = normalize_id("   \t\n", "userId").unwrap_err();
    assert_eq!(err, "userId is required");
}

#[test]
fn normalize_id_trims_and_keeps_body() {
    assert_eq!(normalize_id("  abc  ", "teamId").unwrap(), "abc");
}

#[test]
fn normalize_id_preserves_internal_whitespace() {
    // Only leading/trailing whitespace is stripped — interior is preserved
    // so we don't silently corrupt caller-provided identifiers.
    assert_eq!(normalize_id("a b", "x").unwrap(), "a b");
}
