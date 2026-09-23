use super::*;
use crate::security::{AutonomyLevel, SecurityPolicy};

fn test_security(workspace: std::path::PathBuf) -> Arc<SecurityPolicy> {
    Arc::new(SecurityPolicy {
        autonomy: AutonomyLevel::Supervised,
        workspace_dir: workspace.clone(),
        action_dir: workspace,
        ..SecurityPolicy::default()
    })
}

#[test]
fn apply_patch_name() {
    let tool = ApplyPatchTool::new(test_security(std::env::temp_dir()));
    assert_eq!(tool.name(), "apply_patch");
}

#[tokio::test]
async fn apply_patch_applies_multiple_edits() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_multi");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();
    tokio::fs::write(dir.join("a.txt"), "alpha\nbravo")
        .await
        .unwrap();
    tokio::fs::write(dir.join("b.txt"), "one two")
        .await
        .unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" },
                { "path": "b.txt", "old_string": "two", "new_string": "TWO" }
            ]
        }))
        .await
        .unwrap();
    assert!(!result.is_error, "{}", result.output());
    let a = tokio::fs::read_to_string(dir.join("a.txt")).await.unwrap();
    let b = tokio::fs::read_to_string(dir.join("b.txt")).await.unwrap();
    assert_eq!(a, "ALPHA\nbravo");
    assert_eq!(b, "one TWO");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn apply_patch_atomic_on_validation_failure() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_atomic");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();
    tokio::fs::write(dir.join("a.txt"), "alpha").await.unwrap();
    tokio::fs::write(dir.join("b.txt"), "bravo").await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    // Second edit will fail (no match) — first must NOT be applied.
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" },
                { "path": "b.txt", "old_string": "missing", "new_string": "x" }
            ]
        }))
        .await
        .unwrap();
    assert!(result.is_error);
    let a = tokio::fs::read_to_string(dir.join("a.txt")).await.unwrap();
    assert_eq!(a, "alpha", "atomic: first edit must not be persisted");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn apply_patch_chained_edits_same_file() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_chain");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();
    tokio::fs::write(dir.join("a.txt"), "one two three")
        .await
        .unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "a.txt", "old_string": "one", "new_string": "ONE" },
                { "path": "a.txt", "old_string": "two", "new_string": "TWO" }
            ]
        }))
        .await
        .unwrap();
    assert!(!result.is_error, "{}", result.output());
    let updated = tokio::fs::read_to_string(dir.join("a.txt")).await.unwrap();
    assert_eq!(updated, "ONE TWO three");

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn apply_patch_rejects_empty_edits() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_empty");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool.execute(json!({"edits": []})).await.unwrap();
    assert!(result.is_error);

    let _ = tokio::fs::remove_dir_all(&dir).await;
}

#[tokio::test]
async fn apply_patch_rejects_traversal() {
    let tool = ApplyPatchTool::new(test_security(std::env::temp_dir()));
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "../etc/passwd", "old_string": "x", "new_string": "y" }
            ]
        }))
        .await
        .unwrap();
    assert!(result.is_error);
    assert!(result.output().contains("not allowed"));
}

// -- create mode --------------------------------------------------------------
//
// Empty `old_string` + a path that does not exist = create. Before this the
// tool could only edit, so an agent asked to produce a document had no route:
// the life-scenario `meal-plan` run left two 1-byte files containing `x`,
// placeholders it made with `shell` purely so `apply_patch` had something to
// patch.

#[tokio::test]
async fn apply_patch_creates_a_new_file_from_an_empty_old_string() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_create");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "out/plan.md", "old_string": "", "new_string": "# Plan\nday one\n" }
            ]
        }))
        .await
        .unwrap();
    assert!(!result.is_error, "{}", result.output());
    let written = tokio::fs::read_to_string(dir.join("out/plan.md"))
        .await
        .expect("the file (and its parent) must have been created");
    assert_eq!(written, "# Plan\nday one\n");
    assert!(result.output().contains("created"), "{}", result.output());
}

#[tokio::test]
async fn apply_patch_refuses_an_empty_old_string_on_an_existing_file() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_create_existing");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();
    tokio::fs::write(dir.join("a.txt"), "alpha").await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [{ "path": "a.txt", "old_string": "", "new_string": "overwritten" }]
        }))
        .await
        .unwrap();
    assert!(result.is_error, "{}", result.output());
    // Unchanged — "replace nothing" must never become "replace everything".
    let a = tokio::fs::read_to_string(dir.join("a.txt")).await.unwrap();
    assert_eq!(a, "alpha");
}

#[tokio::test]
async fn apply_patch_mixes_a_create_and_an_edit_in_one_batch() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_create_mixed");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();
    tokio::fs::write(dir.join("a.txt"), "alpha").await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "a.txt", "old_string": "alpha", "new_string": "ALPHA" },
                { "path": "b.txt", "old_string": "", "new_string": "bravo" }
            ]
        }))
        .await
        .unwrap();
    assert!(!result.is_error, "{}", result.output());
    assert_eq!(
        tokio::fs::read_to_string(dir.join("a.txt")).await.unwrap(),
        "ALPHA"
    );
    assert_eq!(
        tokio::fs::read_to_string(dir.join("b.txt")).await.unwrap(),
        "bravo"
    );
}

#[tokio::test]
async fn apply_patch_refuses_two_creates_for_the_same_path() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_create_dup");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [
                { "path": "c.txt", "old_string": "", "new_string": "first" },
                { "path": "c.txt", "old_string": "", "new_string": "second" }
            ]
        }))
        .await
        .unwrap();
    assert!(result.is_error, "{}", result.output());
    assert!(
        !dir.join("c.txt").exists(),
        "a rejected batch must write nothing"
    );
}

#[tokio::test]
async fn apply_patch_create_still_obeys_the_path_policy() {
    let dir = std::env::temp_dir().join("openhuman_test_patch_create_escape");
    let _ = tokio::fs::remove_dir_all(&dir).await;
    tokio::fs::create_dir_all(&dir).await.unwrap();

    let tool = ApplyPatchTool::new(test_security(dir.clone()));
    let result = tool
        .execute(json!({
            "edits": [{ "path": "../escaped.md", "old_string": "", "new_string": "nope" }]
        }))
        .await
        .unwrap();
    assert!(result.is_error, "{}", result.output());
}
