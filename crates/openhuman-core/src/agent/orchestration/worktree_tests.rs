//! Tests for the git-worktree isolation manager.
//!
//! Each test stands up a real temporary git repository (`git init`) so the
//! `git worktree` plumbing is exercised end-to-end. Tests are skipped (pass
//! trivially) when `git` is not on PATH, so CI without git doesn't hard-fail.

use super::*;
use std::path::Path;
use std::process::Command;
use tinyagents_harness::workspace::{
    create_git_worktree, detect_worktree_overlaps, git_worktree_diff_summary, git_worktree_status,
    list_git_worktrees, remove_git_worktree, GitWorktreeBaseRef, GitWorktreeError,
    GitWorktreeStatus,
};

/// `true` when `git` is invokable on this host.
fn git_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git invocation");
    assert!(
        status.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&status.stderr)
    );
}

/// Initialise a temp git repo with one committed file. Returns the tempdir
/// guard (kept alive by the caller) and the repo root path.
fn init_repo() -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().to_path_buf();
    run(&root, &["init", "-b", "main"]);
    run(&root, &["config", "user.email", "test@example.com"]);
    run(&root, &["config", "user.name", "Test User"]);
    std::fs::write(root.join("README.md"), "hello\n").unwrap();
    run(&root, &["add", "README.md"]);
    run(&root, &["commit", "-m", "initial"]);
    (tmp, root)
}

#[test]
fn validate_repo_root_rejects_non_repo() {
    if !git_available() {
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let err = create_git_worktree(tmp.path(), "run-1", GitWorktreeBaseRef::Head).unwrap_err();
    assert!(matches!(err, GitWorktreeError::NotAGitRepo(_)));
}

#[test]
fn create_then_status_reports_clean_worktree() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    let st = create_git_worktree(&root, "run-1", GitWorktreeBaseRef::Head).expect("create");
    assert!(st.path.exists(), "worktree dir should exist");
    assert_eq!(st.branch.as_deref(), Some("worker/run-1"));
    assert!(!st.is_dirty, "fresh worktree is clean");
    assert!(st.changed_files.is_empty());
    assert!(
        st.path.ends_with(Path::new(".claude/worktrees/run-1")),
        "worktree under .claude/worktrees/<run_id>, got {}",
        st.path.display()
    );
}

#[test]
fn list_includes_created_worktree() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    create_git_worktree(&root, "run-a", GitWorktreeBaseRef::Head).expect("create a");
    create_git_worktree(&root, "run-b", GitWorktreeBaseRef::Fresh).expect("create b");
    let all = list_git_worktrees(&root).expect("list");
    // main worktree + the two we created
    assert!(all.len() >= 3, "expected >=3 worktrees, got {}", all.len());
    let branches: Vec<_> = all.iter().filter_map(|w| w.branch.clone()).collect();
    assert!(branches.iter().any(|b| b == "worker/run-a"));
    assert!(branches.iter().any(|b| b == "worker/run-b"));
}

#[test]
fn status_detects_dirty_changes() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    let st = create_git_worktree(&root, "run-dirty", GitWorktreeBaseRef::Head).expect("create");
    // Touch a tracked file + add an untracked one.
    std::fs::write(st.path.join("README.md"), "changed\n").unwrap();
    std::fs::write(st.path.join("new.txt"), "fresh\n").unwrap();

    let st2 = git_worktree_status(&root, &st.path).expect("status");
    assert!(st2.is_dirty, "worktree with edits must be dirty");
    let names: Vec<String> = st2
        .changed_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();
    assert!(names.iter().any(|n| n.contains("README.md")));
    assert!(names.iter().any(|n| n.contains("new.txt")));
}

#[test]
fn diff_summary_lists_tracked_and_untracked() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    let st = create_git_worktree(&root, "run-diff", GitWorktreeBaseRef::Head).expect("create");
    std::fs::write(st.path.join("README.md"), "changed body\n").unwrap();
    std::fs::write(st.path.join("brand_new.txt"), "x\n").unwrap();

    let summary = git_worktree_diff_summary(&root, &st.path).expect("diff");
    assert!(
        summary.contains("README.md"),
        "diff should mention tracked change: {summary}"
    );
    assert!(
        summary.contains("brand_new.txt") && summary.contains("untracked"),
        "diff should list untracked file: {summary}"
    );
}

#[test]
fn remove_refuses_dirty_without_force() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    let st = create_git_worktree(&root, "run-keep", GitWorktreeBaseRef::Head).expect("create");
    std::fs::write(st.path.join("README.md"), "dirty\n").unwrap();

    let err = remove_git_worktree(&root, &st.path, false).expect_err("must refuse dirty");
    assert!(matches!(err, GitWorktreeError::DirtyRefused(_)));
    assert!(st.path.exists(), "dirty worktree must NOT be deleted");
}

#[test]
fn remove_force_deletes_dirty_worktree() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    let st = create_git_worktree(&root, "run-force", GitWorktreeBaseRef::Head).expect("create");
    std::fs::write(st.path.join("README.md"), "dirty\n").unwrap();

    remove_git_worktree(&root, &st.path, true).expect("force remove");
    assert!(!st.path.exists(), "force remove deletes the worktree dir");
}

#[test]
fn remove_clean_worktree_succeeds() {
    if !git_available() {
        return;
    }
    let (_tmp, root) = init_repo();
    let st = create_git_worktree(&root, "run-clean", GitWorktreeBaseRef::Head).expect("create");
    remove_git_worktree(&root, &st.path, false).expect("clean remove");
    assert!(!st.path.exists(), "clean worktree removed without force");
}

#[test]
fn base_ref_parse_defaults_to_head() {
    assert_eq!(GitWorktreeBaseRef::parse(None), GitWorktreeBaseRef::Head);
    assert_eq!(
        GitWorktreeBaseRef::parse(Some("head")),
        GitWorktreeBaseRef::Head
    );
    assert_eq!(
        GitWorktreeBaseRef::parse(Some("HEAD")),
        GitWorktreeBaseRef::Head
    );
    assert_eq!(
        GitWorktreeBaseRef::parse(Some("fresh")),
        GitWorktreeBaseRef::Fresh
    );
    assert_eq!(
        GitWorktreeBaseRef::parse(Some(" Fresh ")),
        GitWorktreeBaseRef::Fresh
    );
    assert_eq!(
        GitWorktreeBaseRef::parse(Some("garbage")),
        GitWorktreeBaseRef::Head
    );
}

// `sanitize_run_id` is TinyAgents-internal now; the identical assertions live
// beside it in `vendor/tinyagents/src/harness/workspace/git/test.rs`. Its effect
// is still observed from this side by the worktree-creation tests below, which
// name their runs and then look for the resulting checkout.

#[test]
fn detect_overlaps_flags_shared_files() {
    let per_worker = vec![
        (
            "w1".to_string(),
            vec![PathBuf::from("src/a.rs"), PathBuf::from("src/b.rs")],
        ),
        (
            "w2".to_string(),
            vec![PathBuf::from("src/b.rs"), PathBuf::from("src/c.rs")],
        ),
        ("w3".to_string(), vec![PathBuf::from("src/c.rs")]),
    ];
    let overlaps = detect_worktree_overlaps(&per_worker);
    // b.rs touched by w1+w2; c.rs touched by w2+w3; a.rs only w1 (no overlap).
    assert_eq!(overlaps.len(), 2);
    assert_eq!(
        overlaps.get(&PathBuf::from("src/b.rs")).unwrap(),
        &vec!["w1".to_string(), "w2".to_string()]
    );
    assert_eq!(
        overlaps.get(&PathBuf::from("src/c.rs")).unwrap(),
        &vec!["w2".to_string(), "w3".to_string()]
    );
    assert!(!overlaps.contains_key(&PathBuf::from("src/a.rs")));
}

#[test]
fn detect_overlaps_empty_when_disjoint() {
    let per_worker = vec![
        ("w1".to_string(), vec![PathBuf::from("a.rs")]),
        ("w2".to_string(), vec![PathBuf::from("b.rs")]),
    ];
    assert!(detect_worktree_overlaps(&per_worker).is_empty());
}

#[test]
fn detect_overlaps_ignores_intra_worker_duplicates() {
    // A single worker listing the same file twice must not self-overlap.
    let per_worker = vec![(
        "w1".to_string(),
        vec![PathBuf::from("a.rs"), PathBuf::from("a.rs")],
    )];
    assert!(detect_worktree_overlaps(&per_worker).is_empty());
}

/// Pins the JSON-RPC wire shape of [`GitWorktreeStatus`].
///
/// `worktree_schemas.rs` serializes this type straight to the desktop UI, so a
/// renamed or dropped field surfaces as an empty worktree panel rather than a
/// failing test. This asserts the exact camelCase key set and value shapes so
/// the type can be re-pointed at the TinyAgents `GitWorktreeStatus` without
/// silently changing the contract.
#[test]
fn worktree_status_serializes_with_stable_camel_case_keys() {
    let status = GitWorktreeStatus {
        path: std::path::PathBuf::from("/tmp/repo/.claude/worktrees/run-1"),
        branch: Some("agent/run-1".to_string()),
        is_dirty: true,
        changed_files: vec![
            std::path::PathBuf::from("src/a.rs"),
            std::path::PathBuf::from("src/b.rs"),
        ],
    };

    let value = serde_json::to_value(&status).expect("GitWorktreeStatus serializes");
    let object = value.as_object().expect("serializes to a JSON object");

    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        vec!["branch", "changedFiles", "isDirty", "path"],
        "worktree status wire keys changed — the desktop worktree panel reads these"
    );

    assert_eq!(value["path"], "/tmp/repo/.claude/worktrees/run-1");
    assert_eq!(value["branch"], "agent/run-1");
    assert_eq!(value["isDirty"], true);
    assert_eq!(
        value["changedFiles"],
        serde_json::json!(["src/a.rs", "src/b.rs"])
    );

    // A detached worktree serializes `branch` as null, not as an omitted key.
    let detached = GitWorktreeStatus {
        branch: None,
        ..status
    };
    let detached_value = serde_json::to_value(&detached).expect("serializes");
    assert!(
        detached_value.as_object().unwrap().contains_key("branch"),
        "branch must stay present-and-null so the UI can distinguish detached HEAD"
    );
    assert_eq!(detached_value["branch"], serde_json::Value::Null);
}
