//! The configured `action_dir` as a granted read-write trusted root.
//!
//! Split out of `policy_trusted_roots_tests.rs` only for file size; the subject
//! is the same grant, and the fixtures there deliberately set
//! `action_dir == workspace_dir`, which is exactly the shape that hid this.

use super::*;
use std::fs;
use std::path::Path as StdPath;

/// The configured `action_dir` must be a read-write trusted root.
///
/// It is the base `validate_path` joins every relative tool path onto, but it
/// was never itself an allow root: the write permission came from the
/// `default_projects_dir()` grant above, which reads `OPENHUMAN_PROJECTS_DIR`
/// and knows nothing about `OPENHUMAN_ACTION_DIR` / `action_dir_override`. On a
/// stock install the two coincide, which is why every existing fixture in this
/// file (`action_dir == workspace_dir`) missed it. Point them apart — which is
/// what the Settings working-folder control does — and a write into the
/// agent's own working directory was refused "Resolved path escapes
/// workspace".
#[test]
fn from_config_grants_the_configured_action_dir_as_readwrite_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("workspace");
    let action = tmp.path().join("elsewhere/action");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(&action).unwrap();

    let cfg = crate::config::AutonomyConfig::default();
    let policy = SecurityPolicy::from_config(&cfg, &workspace, &action);
    let action_str = action.to_string_lossy().to_string();
    assert!(
        policy
            .trusted_roots
            .iter()
            .any(|r| r.path == action_str && matches!(r.access, TrustedAccess::ReadWrite)),
        "from_config must grant the action dir as read-write; got: {:?}",
        policy.trusted_roots
    );
}

/// The grant must reach the gate every file write funnels through, not just the
/// `trusted_roots` vector. Built with the policy explicitly enabled, since the
/// containment this proves is exactly what `enabled = false` turns off.
#[tokio::test]
async fn an_enabled_policy_can_write_into_its_action_dir_outside_the_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("workspace");
    let action = tmp.path().join("elsewhere/action");
    fs::create_dir_all(&workspace).unwrap();
    fs::create_dir_all(action.join("out")).unwrap();

    let cfg = crate::config::AutonomyConfig {
        enabled: true,
        ..crate::config::AutonomyConfig::default()
    };
    let policy = SecurityPolicy::from_config(&cfg, &workspace, &action);
    let resolved = policy
        .validate_parent_path("out/plan.md")
        .await
        .expect("a relative write into the action dir must be permitted");
    assert!(resolved.ends_with("out/plan.md"), "got {resolved:?}");
}

/// An action dir at or above `workspace_dir` is NOT granted: a trusted root
/// there would hand the whole workspace a `forbidden_paths` bypass
/// (`check_resolved_against_forbidden` returns early for a trusted root).
#[test]
fn from_config_does_not_grant_an_action_dir_covering_the_workspace() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let action = tmp.path().to_path_buf();
    let workspace = action.join("workspace");
    fs::create_dir_all(&workspace).unwrap();

    let cfg = crate::config::AutonomyConfig::default();
    let policy = SecurityPolicy::from_config(&cfg, &workspace, &action);
    let action_str = action.to_string_lossy().to_string();
    assert!(
        !policy.trusted_roots.iter().any(|r| r.path == action_str),
        "an action dir above the workspace must not be granted; got: {:?}",
        policy.trusted_roots
    );
}
