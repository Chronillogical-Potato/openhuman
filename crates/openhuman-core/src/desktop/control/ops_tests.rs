use super::*;

#[test]
fn disabled_by_default_and_persists() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    assert!(!enabled(&config));
    save(&config, true).unwrap();
    assert!(enabled(&config));
    save(&config, false).unwrap();
    assert!(!enabled(&config));
}

#[test]
fn permission_states_are_normalized() {
    let data = serde_json::json!({"accessibility":{"state":"granted"},
            "screen_recording":{"state":"weird"}});
    assert_eq!(permission(&data, "accessibility"), "granted");
    assert_eq!(permission(&data, "screen_recording"), "unknown");
}

#[test]
fn approval_bypass_requires_a_registered_desktop_tool_and_disabled_setting() {
    use super::super::tools::{DesktopTool, DesktopToolKind};
    let mut config = Config::default();
    let goal = DesktopTool::new(std::sync::Arc::new(config.clone()), DesktopToolKind::Goal);
    let raw_action = DesktopTool::new(std::sync::Arc::new(config.clone()), DesktopToolKind::Act);
    assert!(approval_bypass_decision(&config, &goal));
    assert!(!approval_bypass_decision(&config, &raw_action));
    config.desktop.approvals_enabled = true;
    assert!(!approval_bypass_decision(&config, &goal));
}
