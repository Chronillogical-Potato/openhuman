use super::*;

#[test]
fn forgetting_keeps_a_root_recorded_meanwhile() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record(dir, "c", "root").unwrap();
    let seen = of(dir, "c").unwrap();
    // A sync records another root while the forget runs.
    record(dir, "c", "team:new").unwrap();
    forget(dir, "c", &seen);
    assert_eq!(
        of(dir, "c").unwrap(),
        BTreeSet::from(["team:new".to_string()])
    );
    forget(dir, "c", &of(dir, "c").unwrap());
    assert!(of(dir, "c").unwrap().is_empty());
}

#[test]
fn an_unparsable_file_is_set_aside_not_overwritten() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let file = path(dir);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "{ not json").unwrap();
    assert!(of(dir, "c").unwrap().is_empty());
    assert_eq!(
        std::fs::read_to_string(file.with_extension("json.corrupt")).unwrap(),
        "{ not json"
    );
    record(dir, "c", "root").unwrap();
    assert_eq!(of(dir, "c").unwrap().len(), 1, "recording resumed");
}

#[test]
fn a_record_that_cannot_be_written_is_an_error() {
    let tmp = tempfile::tempdir().unwrap();
    // `memory` is a file, so the record's directory cannot be made.
    std::fs::write(tmp.path().join("memory"), "").unwrap();
    assert!(record(tmp.path(), "c", "root").is_err());
}

#[test]
fn an_unreadable_file_is_left_alone_and_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record(dir, "c", "root").unwrap();
    // A directory where the file should be cannot be read as text.
    let file = path(dir);
    std::fs::remove_file(&file).unwrap();
    std::fs::create_dir(&file).unwrap();
    assert!(of(dir, "c").is_none(), "the caller must search everywhere");
    assert!(
        record(dir, "c", "team:new").is_err(),
        "nothing is filed unrecorded"
    );
    assert!(file.is_dir(), "nothing was set aside or overwritten");
}
