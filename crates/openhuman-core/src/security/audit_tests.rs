use super::*;
use tempfile::TempDir;

#[test]
fn audit_event_new_creates_unique_id() {
    let event1 = AuditEvent::new(AuditEventType::CommandExecution);
    let event2 = AuditEvent::new(AuditEventType::CommandExecution);
    assert_ne!(event1.event_id, event2.event_id);
}

#[test]
fn audit_event_with_actor() {
    let event = AuditEvent::new(AuditEventType::CommandExecution).with_actor(
        "telegram".to_string(),
        Some("123".to_string()),
        Some("@alice".to_string()),
    );

    assert!(event.actor.is_some());
    let actor = event.actor.as_ref().unwrap();
    assert_eq!(actor.channel, "telegram");
    assert_eq!(actor.user_id, Some("123".to_string()));
    assert_eq!(actor.username, Some("@alice".to_string()));
}

#[test]
fn audit_event_with_action() {
    let event = AuditEvent::new(AuditEventType::CommandExecution).with_action(
        "ls -la".to_string(),
        "low".to_string(),
        false,
        true,
    );

    assert!(event.action.is_some());
    let action = event.action.as_ref().unwrap();
    assert_eq!(action.command, Some("ls -la".to_string()));
    assert_eq!(action.risk_level, Some("low".to_string()));
}

#[test]
fn audit_event_serializes_to_json() {
    let event = AuditEvent::new(AuditEventType::CommandExecution)
        .with_actor("telegram".to_string(), None, None)
        .with_action("ls".to_string(), "low".to_string(), false, true)
        .with_result(true, Some(0), 15, None);

    let json = serde_json::to_string(&event);
    assert!(json.is_ok());
    let json = json.expect("serialize");
    let parsed: AuditEvent = serde_json::from_str(json.as_str()).expect("parse");
    assert!(parsed.actor.is_some());
    assert!(parsed.action.is_some());
    assert!(parsed.result.is_some());
}

#[test]
fn audit_logger_disabled_helper_is_noop() -> Result<()> {
    let logger = AuditLogger::disabled();
    let event = AuditEvent::new(AuditEventType::CommandExecution);
    logger.log(&event)?;
    assert!(!logger.config.enabled);
    Ok(())
}

#[test]
fn workspace_audit_logger_is_shared_per_workspace() -> Result<()> {
    let tmp = TempDir::new()?;
    let cfg = AuditConfig::default();
    let first = get_or_create_workspace_audit_logger(cfg.clone(), tmp.path().to_path_buf())?;
    let second = get_or_create_workspace_audit_logger(cfg, tmp.path().to_path_buf())?;
    assert!(
        Arc::ptr_eq(&first, &second),
        "same workspace must yield the same shared logger instance"
    );
    Ok(())
}

#[test]
fn audit_logger_disabled_does_not_create_file() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: false,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;
    let event = AuditEvent::new(AuditEventType::CommandExecution);

    logger.log(&event)?;

    // File should not exist since logging is disabled
    assert!(!tmp.path().join("audit.log").exists());
    Ok(())
}

// ── §8.1 Log rotation tests ─────────────────────────────

#[tokio::test]
async fn audit_logger_writes_event_when_enabled() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 10,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;
    let event = AuditEvent::new(AuditEventType::CommandExecution)
        .with_actor("cli".to_string(), None, None)
        .with_action("ls".to_string(), "low".to_string(), false, true);

    logger.log(&event)?;

    let log_path = tmp.path().join("audit.log");
    assert!(log_path.exists(), "audit log file must be created");

    let content = tokio::fs::read_to_string(&log_path).await?;
    assert!(!content.is_empty(), "audit log must not be empty");

    let parsed: AuditEvent = serde_json::from_str(content.trim())?;
    assert!(parsed.action.is_some());
    Ok(())
}

#[tokio::test]
async fn audit_log_command_event_writes_structured_entry() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 10,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;

    logger.log_command_event(CommandExecutionLog {
        channel: "telegram",
        command: "echo test",
        risk_level: "low",
        approved: false,
        allowed: true,
        success: true,
        duration_ms: 42,
    })?;

    let log_path = tmp.path().join("audit.log");
    let content = tokio::fs::read_to_string(&log_path).await?;
    let parsed: AuditEvent = serde_json::from_str(content.trim())?;

    let action = parsed.action.unwrap();
    assert!(action.command.is_some());
    assert_eq!(action.risk_level, Some("low".to_string()));
    assert!(action.allowed);

    let result = parsed.result.unwrap();
    assert!(result.success);
    assert_eq!(result.duration_ms, Some(42));
    Ok(())
}

#[test]
fn audit_log_command_event_redacts_command_before_persisting() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 10,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;
    let secret = "credential-that-must-not-be-logged";

    logger.log_command_event(CommandExecutionLog {
        channel: "cli",
        command: secret,
        risk_level: "high",
        approved: false,
        allowed: true,
        success: true,
        duration_ms: 1,
    })?;

    let content = std::fs::read_to_string(tmp.path().join("audit.log"))?;
    assert!(
        !content.contains(secret),
        "command secrets must not reach disk"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn audit_log_file_is_created_owner_only() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 10,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;
    logger.log(&AuditEvent::new(AuditEventType::SecurityEvent))?;

    let mode = std::fs::metadata(tmp.path().join("audit.log"))?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, 0o600, "audit log must be readable only by its owner");
    Ok(())
}

#[cfg(unix)]
#[test]
fn audit_log_existing_permissive_file_is_tightened() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new()?;
    let log_path = tmp.path().join("audit.log");
    std::fs::write(&log_path, "existing\n")?;
    std::fs::set_permissions(&log_path, std::fs::Permissions::from_mode(0o644))?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 10,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;

    logger.log(&AuditEvent::new(AuditEventType::SecurityEvent))?;

    let mode = std::fs::metadata(log_path)?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "existing audit log permissions must be tightened");
    Ok(())
}

#[cfg(unix)]
#[test]
fn audit_event_is_appended_when_rotation_fails_and_error_is_reported() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 0,
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;
    let log_path = tmp.path().join("audit.log");
    std::fs::write(&log_path, "existing\n")?;
    // Keep each rotation destination blocked so the final rename of the
    // current log also fails instead of moving the blockers out of the way.
    for index in 1..=10 {
        let blocker = std::path::PathBuf::from(format!("{}.{}.log", log_path.display(), index));
        std::fs::create_dir(&blocker)?;
        std::fs::write(blocker.join("blocker"), "keep directory non-empty")?;
    }

    let result = logger.log_command_event(CommandExecutionLog {
        channel: "cli",
        command: "echo event-survives-rotation",
        risk_level: "low",
        approved: false,
        allowed: true,
        success: true,
        duration_ms: 1,
    });

    assert!(result.is_err(), "rotation error must remain observable");
    let content = std::fs::read_to_string(log_path)?;
    assert!(
        content.contains("command_execution"),
        "event must still be appended"
    );
    Ok(())
}

#[test]
fn audit_rotation_creates_numbered_backup() -> Result<()> {
    let tmp = TempDir::new()?;
    let config = AuditConfig {
        enabled: true,
        max_size_mb: 0, // Force rotation on first write
        ..Default::default()
    };
    let logger = AuditLogger::new(config, tmp.path().to_path_buf())?;

    // Write initial content that triggers rotation
    let log_path = tmp.path().join("audit.log");
    std::fs::write(&log_path, "initial content\n")?;

    let event = AuditEvent::new(AuditEventType::CommandExecution);
    logger.log(&event)?;

    let rotated = format!("{}.1.log", log_path.display());
    assert!(
        std::path::Path::new(&rotated).exists(),
        "rotation must create .1.log backup"
    );
    Ok(())
}
