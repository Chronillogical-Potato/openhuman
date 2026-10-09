use super::*;

const LOCAL: &str = "http://127.0.0.1:3141";

#[test]
fn one_endpoint_has_one_marker_however_it_is_spelled() {
    let dir = Path::new("/app");
    let key = ClaimKey::new(dir, LOCAL, "user:a");
    for spelling in [
        " http://127.0.0.1:3141/ ",
        "HTTP://127.0.0.1:3141",
        "http://127.0.0.1:3141/",
    ] {
        assert_eq!(ClaimKey::new(dir, spelling, "user:b").marker, key.marker);
    }
    assert_ne!(
        ClaimKey::new(dir, "http://127.0.0.1:3142", "user:a").marker,
        key.marker
    );
}

#[test]
fn the_first_account_takes_the_tree_and_keeps_it() {
    let tmp = tempfile::tempdir().unwrap();
    let a = ClaimKey::new(tmp.path(), LOCAL, "user:a");
    let b = ClaimKey::new(tmp.path(), LOCAL, "user:b");
    assert!(!held_by_other(&a).unwrap());
    assert!(take(&a).unwrap());
    assert!(take(&a).unwrap(), "taking again is a no-op");
    assert!(!held_by_other(&a).unwrap());
    assert!(held_by_other(&b).unwrap());
    assert!(!take(&b).unwrap(), "never replaces another's claim");
    assert!(!held_by_other(&a).unwrap());
}

#[test]
fn an_unreadable_marker_is_an_error_not_a_free_tree() {
    let tmp = tempfile::tempdir().unwrap();
    let a = ClaimKey::new(tmp.path(), LOCAL, "user:a");
    std::fs::create_dir_all(a.marker.parent().unwrap()).unwrap();
    std::fs::write(&a.marker, b"{torn").unwrap();
    assert!(held_by_other(&a).is_err());
    assert!(take(&a).is_err());
}
