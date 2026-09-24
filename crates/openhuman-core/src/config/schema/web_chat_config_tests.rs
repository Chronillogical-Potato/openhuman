use super::WebChatConfig;

#[test]
fn defaults_to_suggestions_enabled() {
    let config = WebChatConfig::default();
    assert!(config.suggestions_enabled);
}

#[test]
fn deserializes_missing_field_as_enabled() {
    let config: WebChatConfig = serde_json::from_str("{}").unwrap();
    assert!(config.suggestions_enabled);
}

#[test]
fn deserializes_explicit_false() {
    let config: WebChatConfig =
        serde_json::from_str(r#"{"suggestions_enabled": false}"#).unwrap();
    assert!(!config.suggestions_enabled);
}
