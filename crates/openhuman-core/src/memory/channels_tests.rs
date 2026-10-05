use super::*;

#[test]
fn records_threads_per_channel_case_insensitively() {
    let tmp = tempfile::tempdir().unwrap();
    record(tmp.path(), "Telegram", "t-1");
    record(tmp.path(), "telegram", "t-1");
    record(tmp.path(), "telegram", "t-2");
    record(tmp.path(), "", "t-3");
    record(tmp.path(), "web", " ");
    assert_eq!(threads_of(tmp.path(), "TELEGRAM"), ["t-1", "t-2"]);
    assert!(threads_of(tmp.path(), "web").is_empty());
}
