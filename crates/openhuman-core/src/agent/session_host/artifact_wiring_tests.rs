use super::*;
use crate::security::policy::tool_result_artifacts_dir;

const DAY: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

fn age(path: &Path, by: std::time::Duration) {
    let old = std::time::SystemTime::now() - by;
    let file = std::fs::File::options().write(true).open(path).unwrap();
    file.set_modified(old).unwrap();
}

#[cfg(windows)]
fn open_directory(path: &Path) -> std::fs::File {
    use std::os::windows::fs::OpenOptionsExt;
    std::fs::File::options()
        .read(true)
        .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
        .unwrap()
}

#[cfg(not(windows))]
fn open_directory(path: &Path) -> std::fs::File {
    std::fs::File::open(path).unwrap()
}

fn stale_session(root: &Path, session: &str) -> PathBuf {
    let dir = root.join(session).join("shell");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("call.txt");
    std::fs::write(&file, "old output").unwrap();
    age(&file, 2 * DAY);
    // A directory's own mtime counts toward freshness too.
    for d in [dir.as_path(), dir.parent().unwrap()] {
        let handle = open_directory(d);
        handle
            .set_modified(std::time::SystemTime::now() - 2 * DAY)
            .unwrap();
    }
    root.join(session)
}

/// The store writes under the workspace, never the action directory, whether
/// or not the turn carries a workspace descriptor.
#[test]
fn the_store_is_rooted_in_the_workspace_not_the_action_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().join("workspace");
    let action = tmp.path().join("project");
    let descriptor = tinytools::WorkspaceDescriptor {
        root: tmp.path().join("checkout"),
        trusted_roots: Vec::new(),
        policy_id: String::new(),
        sandbox: Default::default(),
    };

    for descriptor in [None, Some(&descriptor)] {
        let store = build_artifact_store(&workspace, descriptor, &action, "s");
        assert!(store.is_detached());
        assert_eq!(store.root(), tool_result_artifacts_dir(&workspace));
        let pointer = store.path_for_read_tool("shell", Some("c"));
        assert!(Path::new(&pointer).starts_with(workspace.join("artifacts/tool-results")));
    }
}

/// Artifacts older builds left in the project keep being swept with the age
/// rule they always had, while a fresh one (a concurrent older session) and the
/// current session are left alone.
#[test]
fn stale_artifacts_left_in_the_project_by_older_builds_are_swept() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace = tmp.path().join("workspace");
    let action = tmp.path().join("project");
    let legacy_root = action.join("artifacts/tool-results");
    let stale = stale_session(&legacy_root, "old");
    let fresh = legacy_root.join("recent/shell");
    std::fs::create_dir_all(&fresh).unwrap();
    std::fs::write(fresh.join("call.txt"), "recent").unwrap();
    let detached_stale = stale_session(&tool_result_artifacts_dir(&workspace), "older");

    build_artifact_store(&workspace, None, &action, "current");

    assert!(!stale.exists(), "a stale legacy session is swept");
    assert!(fresh.join("call.txt").exists(), "a fresh one is kept");
    assert!(!detached_stale.exists(), "the workspace store is swept too");
}

#[test]
fn legacy_roots_include_a_distinct_descriptor_root_once() {
    let action = PathBuf::from("/p/action");
    let descriptor = |root: &str| tinytools::WorkspaceDescriptor {
        root: PathBuf::from(root),
        trusted_roots: Vec::new(),
        policy_id: String::new(),
        sandbox: Default::default(),
    };
    assert_eq!(legacy_roots(None, &action), vec![action.clone()]);
    assert_eq!(
        legacy_roots(Some(&descriptor("/p/action")), &action),
        vec![action.clone()]
    );
    assert_eq!(
        legacy_roots(Some(&descriptor("/p/checkout")), &action),
        vec![action.clone(), PathBuf::from("/p/checkout")]
    );
}
