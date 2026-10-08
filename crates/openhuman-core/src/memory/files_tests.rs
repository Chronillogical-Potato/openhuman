use super::*;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn write_private_writes_the_bytes() {
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("memory").join("state.json");
    write_private(&file, b"{\"a\":1}").unwrap();
    write_private(&file, b"{}").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"{}", "truncated, not appended");
}

#[cfg(unix)]
#[test]
fn a_new_file_is_0600_in_a_0700_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("memory").join("nested");
    let file = dir.join("state.json");
    write_private(&file, b"{}").unwrap();
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&tmp.path().join("memory")), 0o700);
}

#[cfg(unix)]
#[test]
fn an_existing_world_readable_file_and_directory_are_narrowed() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("memory");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    let file = dir.join("jobs.json");
    std::fs::write(&file, b"{}").unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();

    write_private(&file, b"{\"pending\":[]}").unwrap();
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&dir), 0o700);
}

#[cfg(unix)]
#[tokio::test]
async fn every_memory_state_file_is_owner_only() {
    use tinymemory_api::{ConsolidateRequest, ItemKind, Namespace, Reach};

    use crate::memory::lifecycle::jobs::{self, BackgroundJob};
    use crate::memory::test_fixtures::config_in;

    let tmp = tempfile::tempdir().unwrap();
    let config = config_in(&tmp);
    let workspace = config.workspace_dir.clone();

    crate::memory::channels::record(&workspace, "telegram", "thread-1");
    crate::memory::sources::state::update(&workspace, "src-1", |_| {});
    crate::memory::sources::roots::record(&workspace, "composio:gmail", "user:x").unwrap();
    crate::memory::layout_migration::state::save(&workspace, &Default::default()).unwrap();
    jobs::enqueue(
        &config,
        &Namespace::ROOT,
        vec![BackgroundJob::BuildBeliefs {
            request: ConsolidateRequest::new(Reach::exact(Namespace::agent("a")))
                .kinds([ItemKind::Conversation]),
        }],
    )
    .await;

    let dir = workspace.join("memory");
    assert_eq!(mode(&dir), 0o700);
    let mut seen = 0;
    for entry in std::fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        assert_eq!(mode(&path), 0o600, "{}", path.display());
        seen += 1;
    }
    assert!(seen >= 5, "every state file was written: {seen}");
}
