use super::*;

fn pair(key: &str, id: &str) -> (String, String) {
    (key.to_string(), id.to_string())
}

#[test]
fn a_changed_record_hands_back_its_previous_id() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    assert!(begin(dir, &[pair("k1", "a"), pair("k2", "b")], &[]).is_empty());

    // Unchanged, edited and new records: only the edited one's old id.
    let stale = begin(
        dir,
        &[pair("k1", "a"), pair("k2", "b2"), pair("k3", "c")],
        &[],
    );
    assert_eq!(stale, ["b"]);
    settle(dir, &stale);
    assert!(begin(dir, &[pair("k2", "b2")], &[]).is_empty());
}

#[test]
fn an_unsettled_id_is_handed_back_again_and_the_new_one_is_kept() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let _ = begin(dir, &[pair("k", "v1")], &[]);
    // The forget of v1 fails: nothing is settled.
    assert_eq!(begin(dir, &[pair("k", "v2")], &[]), ["v1"]);
    // Edited again before the retry: v2 is stale too, v1 is still owed.
    assert_eq!(begin(dir, &[pair("k", "v3")], &[]), ["v1", "v2"]);
    settle(dir, &["v1".into(), "v2".into()]);
    assert!(begin(dir, &[pair("k", "v3")], &[]).is_empty());
}

#[test]
fn an_id_another_record_still_holds_is_not_stale() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    // Two records with the same content share an id.
    let _ = begin(dir, &[pair("a", "x"), pair("b", "x")], &[]);
    assert!(
        begin(dir, &[pair("a", "y")], &[]).is_empty(),
        "b still holds x"
    );
    assert_eq!(begin(dir, &[pair("b", "z")], &[]), ["x"]);
}

#[test]
fn a_record_that_comes_back_empty_hands_back_its_id() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let _ = begin(dir, &[pair("k", "v1")], &[]);
    assert_eq!(begin(dir, &[], &["k".into()]), ["v1"]);
}

#[test]
fn keys_do_not_collide_across_connections() {
    assert_ne!(key("a:b", "c"), key("a", "b:c"));
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let _ = begin(
        dir,
        &[pair(&key("a:b", "c"), "x"), pair(&key("a", "b:c"), "y")],
        &[],
    );
    drop_connection(dir, "a");
    // `a:b`'s record is untouched by dropping `a`.
    let stale = begin(dir, &[pair(&key("a:b", "c"), "x2")], &[]);
    assert_eq!(stale, ["x"]);
    settle(dir, &stale);
    // `a`'s record was dropped, so its new id replaces nothing.
    assert!(begin(dir, &[pair(&key("a", "b:c"), "y2")], &[]).is_empty());
}

#[test]
fn an_unparsable_file_is_set_aside_not_overwritten() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let file = path(dir);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{ not json").unwrap();
    assert!(begin(dir, &[pair("k", "v")], &[]).is_empty());
    assert_eq!(
        std::fs::read_to_string(file.with_extension("json.corrupt")).unwrap(),
        "{ not json"
    );
    assert_eq!(
        begin(dir, &[pair("k", "v2")], &[]),
        ["v"],
        "tracking resumed"
    );
}

#[test]
fn a_pass_with_no_records_still_hands_back_pending_ids() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let _ = begin(dir, &[pair("k", "v1")], &[]);
    let _ = begin(dir, &[pair("k", "v2")], &[]); // the forget of v1 fails
    assert_eq!(begin(dir, &[], &[]), ["v1"]);
}

#[test]
fn an_unreadable_file_changes_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let _ = begin(dir, &[pair("k", "v1")], &[]);
    // A directory where the file should be cannot be read as text.
    let file = path(dir);
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(begin(dir, &[pair("k", "v2")], &[]).is_empty());
    assert!(file.is_dir(), "nothing was set aside or overwritten");
}
