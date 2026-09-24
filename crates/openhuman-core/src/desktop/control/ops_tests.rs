use super::*;

#[cfg(any(target_os = "macos", target_os = "windows"))]
static TEST_LOOPBACK_LOCK: Mutex<()> = Mutex::new(());

#[cfg(any(target_os = "macos", target_os = "windows"))]
struct LoopbackTestGuard {
    previous: bool,
    _lock: std::sync::MutexGuard<'static, ()>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl LoopbackTestGuard {
    fn enable() -> Self {
        let lock = TEST_LOOPBACK_LOCK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        Self {
            previous: LOOPBACK_LISTENER.swap(true, Ordering::AcqRel),
            _lock: lock,
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Drop for LoopbackTestGuard {
    fn drop(&mut self) {
        set_listener_is_loopback(self.previous);
    }
}

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
fn unreadable_or_corrupt_state_never_enables_desktop() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    let path = state_path(&config);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "not json").unwrap();
    assert!(!enabled(&config));
    std::fs::write(&path, r#"{"enabled":"true"}"#).unwrap();
    assert!(!enabled(&config));
    std::fs::remove_file(&path).unwrap();
    std::fs::create_dir(&path).unwrap();
    assert!(!enabled(&config));
    assert!(save(&config, true).is_err());
    assert!(!enabled(&config));
}

#[tokio::test]
async fn disabled_probe_never_calls_the_module() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    let result = probe_with(&config, |_| async {
        panic!("disabled probe reached module")
    })
    .await;
    assert!(!result.ok);
    assert_eq!(result.app_count, None);
    assert_eq!(
        result.reason.as_deref(),
        Some("desktop control is unavailable or disabled")
    );
}

#[tokio::test]
async fn disabled_status_reports_local_setting_without_contacting_module() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    let result = status_with(&config, || async {
        panic!("disabled status reached module")
    })
    .await;
    assert!(!result.enabled);
    assert!(!result.approvals_enabled);
    assert_eq!(result.accessibility, "unknown");
    assert_eq!(result.screen_recording, "unknown");
    assert_eq!(result.platform, std::env::consts::OS);
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[tokio::test]
async fn enabled_status_reports_module_permissions_and_errors() {
    use tinydesktop_bus::{DesktopError, DesktopResponse};

    let _loopback = LoopbackTestGuard::enable();
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    save(&config, true).unwrap();

    let permitted = status_with(&config, || async {
        Ok(DesktopResponse::ok(
            "permissions",
            serde_json::json!({
                "accessibility":{"state":"granted"},
                "screen_recording":{"state":"not_required"}
            }),
        ))
    })
    .await;
    assert!(permitted.supported && permitted.enabled);
    assert_eq!(permitted.accessibility, "granted");
    assert_eq!(permitted.screen_recording, "not_required");

    let refused = status_with(&config, || async {
        Ok(DesktopResponse::err(
            "permissions",
            DesktopError::new("PERM_DENIED", "Grant Accessibility"),
        ))
    })
    .await;
    assert_eq!(refused.reason.as_deref(), Some("Grant Accessibility"));
    assert_eq!(refused.accessibility, "unknown");

    let unavailable = status_with(&config, || async {
        Err("module could not start".to_owned())
    })
    .await;
    assert_eq!(
        unavailable.reason.as_deref(),
        Some("module could not start")
    );
}

#[tokio::test]
async fn disabling_desktop_persists_even_if_state_was_enabled() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    save(&config, true).unwrap();
    let result = set_enabled(&config, false).await.unwrap();
    assert!(!result.enabled);
    assert!(!enabled(&config));
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[tokio::test]
async fn probe_requires_accessibility_and_snapshot_before_listing_apps() {
    use tinydesktop_bus::{DesktopError, DesktopResponse};

    let _loopback = LoopbackTestGuard::enable();
    let dir = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.workspace_dir = dir.path().to_path_buf();
    save(&config, true).unwrap();

    let denied = probe_with(&config, |member| async move {
        assert_eq!(member, names::methods::PERMISSIONS);
        Ok(DesktopResponse::ok(
            member,
            serde_json::json!({
                "accessibility":{"state":"denied"}
            }),
        ))
    })
    .await;
    assert!(!denied.ok);
    assert_eq!(
        denied.reason.as_deref(),
        Some("Accessibility permission is not granted to the core process")
    );

    let failed = probe_with(&config, |member| async move {
        match member {
            names::methods::PERMISSIONS => Ok(DesktopResponse::ok(
                member,
                serde_json::json!({
                    "accessibility":{"state":"granted"}
                }),
            )),
            names::methods::SNAPSHOT => Ok(DesktopResponse::err(
                member,
                DesktopError::new("PERM_DENIED", "Screen Recording required"),
            )),
            _ => panic!("failed snapshot must prevent app listing"),
        }
    })
    .await;
    assert!(!failed.ok);
    assert_eq!(failed.reason.as_deref(), Some("Screen Recording required"));

    let success = probe_with(&config, |member| async move {
        Ok(DesktopResponse::ok(
            member,
            match member {
                names::methods::PERMISSIONS => {
                    serde_json::json!({"accessibility":{"state":"granted"}})
                }
                names::methods::SNAPSHOT => serde_json::json!({"elements":[]}),
                names::methods::LIST_APPS => {
                    serde_json::json!({"apps":[{"name":"TextEdit"},{"name":"Finder"}]})
                }
                _ => panic!("unexpected member"),
            },
        ))
    })
    .await;
    assert!(success.ok);
    assert_eq!(success.app_count, Some(2));
    assert!(success.reason.is_none());

    let transport_error = probe_with(&config, |member| async move {
        match member {
            names::methods::PERMISSIONS => Ok(DesktopResponse::ok(
                member,
                serde_json::json!({"accessibility":{"state":"granted"}}),
            )),
            names::methods::SNAPSHOT => Ok(DesktopResponse::ok(
                member,
                serde_json::json!({"elements":[]}),
            )),
            names::methods::LIST_APPS => Err("app list bus disconnected".to_owned()),
            _ => panic!("unexpected member"),
        }
    })
    .await;
    assert!(!transport_error.ok);
    assert_eq!(transport_error.app_count, None);
    assert_eq!(
        transport_error.reason.as_deref(),
        Some("app list bus disconnected")
    );

    let module_error = probe_with(&config, |member| async move {
        match member {
            names::methods::PERMISSIONS => Ok(DesktopResponse::ok(
                member,
                serde_json::json!({"accessibility":{"state":"granted"}}),
            )),
            names::methods::SNAPSHOT => Ok(DesktopResponse::ok(
                member,
                serde_json::json!({"elements":[]}),
            )),
            names::methods::LIST_APPS => Ok(DesktopResponse::err(
                member,
                DesktopError::new("PERM_DENIED", "App listing denied"),
            )),
            _ => panic!("unexpected member"),
        }
    })
    .await;
    assert!(!module_error.ok);
    assert_eq!(module_error.app_count, None);
    assert_eq!(module_error.reason.as_deref(), Some("App listing denied"));
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
