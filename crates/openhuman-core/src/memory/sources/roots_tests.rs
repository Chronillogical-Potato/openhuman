use super::*;

#[test]
fn forgetting_keeps_a_root_recorded_meanwhile() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record(dir, "c", "root").unwrap();
    let seen = of(dir, "c");
    // A sync records another root while the forget runs.
    record(dir, "c", "team:new").unwrap();
    forget(dir, "c", &seen);
    assert_eq!(of(dir, "c"), BTreeSet::from(["team:new".to_string()]));
    forget(dir, "c", &of(dir, "c"));
    assert!(of(dir, "c").is_empty());
}

#[test]
fn an_unparsable_file_is_set_aside_not_overwritten() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let file = path(dir);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{ not json").unwrap();
    assert!(of(dir, "c").is_empty());
    assert_eq!(
        std::fs::read_to_string(file.with_extension("json.corrupt")).unwrap(),
        "{ not json"
    );
    record(dir, "c", "root").unwrap();
    assert_eq!(of(dir, "c").len(), 1, "recording resumed");
}

#[test]
fn a_record_that_cannot_be_written_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    // `memory` is a file, so the record's directory cannot be made.
    std::fs::write(tmp.path().join("memory"), "").unwrap();
    assert!(record(tmp.path(), "c", "root").is_err());
}
