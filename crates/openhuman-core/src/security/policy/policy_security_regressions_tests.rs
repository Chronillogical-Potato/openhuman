use super::*;
use crate::security::policy::CommandAnalysis;

#[test]
fn supervised_analysis_unquotes_embedded_quotes_in_find_exec_flag() {
    use tinybox_core::shell::scan;

    let mut policy = default_policy();
    policy.autonomy = AutonomyLevel::Supervised;
    policy.allowed_commands = vec!["find".into()];
    let command = "find . -e''xec rm {} ;";
    let facts = tinybox_bus::ShellAnalysis {
        segments: scan::split_unquoted_segments(command),
        redirection: false,
    };
    let analysis = CommandAnalysis::from_shell_analysis(facts);

    assert_eq!(analysis.segments[0].arguments[1], "-exec");
    assert!(!policy.is_command_allowed_analysis(&analysis));
}

#[test]
fn file_url_literals_still_apply_the_protected_path_floor() {
    let policy = SecurityPolicy::default();

    let result = policy.check_protected_path_literals("cat file:///home/operator/.ssh/id_rsa");

    assert!(
        result.is_err(),
        "file URL to credential path must be denied"
    );
}

#[cfg(unix)]
#[test]
fn symlink_check_uses_action_dir_for_relative_paths() {
    let workspace = tempfile::tempdir().unwrap();
    let action = tempfile::tempdir().unwrap();
    let link = action.path().join("link");
    std::os::unix::fs::symlink("/etc/hostname", &link).unwrap();
    let policy = SecurityPolicy {
        enabled: true,
        workspace_only: false,
        workspace_dir: workspace.path().to_path_buf(),
        action_dir: action.path().to_path_buf(),
        forbidden_paths: vec!["/etc".into()],
        ..SecurityPolicy::default()
    };

    assert!(
        !policy.is_path_string_allowed("link"),
        "relative symlink resolution must start at action_dir, as validate_path does"
    );
}

#[cfg(windows)]
#[test]
fn symlink_forbidden_paths_with_tilde_use_platform_home_resolution() {
    let home = dirs::home_dir().expect("Windows home directory is available");
    let forbidden = home.join("openhuman-policy-regression");
    std::fs::create_dir_all(&forbidden).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let action = workspace.path().join("action");
    std::fs::create_dir_all(&action).unwrap();
    let link = action.join("link");
    std::os::windows::fs::symlink_dir(&forbidden, &link).unwrap();
    let policy = SecurityPolicy {
        enabled: true,
        workspace_only: false,
        workspace_dir: workspace.path().to_path_buf(),
        action_dir: action,
        forbidden_paths: vec!["~/openhuman-policy-regression".into()],
        ..SecurityPolicy::default()
    };

    assert!(!policy.is_path_string_allowed("link/secret.txt"));
    std::fs::remove_dir_all(forbidden).unwrap();
}
