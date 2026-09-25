use super::*;

#[test]
fn approvals_are_off_by_default_and_can_be_opted_in() {
    assert!(!DesktopConfig::default().approvals_enabled);
    let enabled: DesktopConfig = toml::from_str("approvals_enabled = true").unwrap();
    assert!(enabled.approvals_enabled);
}
