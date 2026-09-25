use super::*;

#[test]
fn default_engine_is_local() {
    assert_eq!(
        SubconsciousConfig::default().engine,
        SubconsciousEngine::Local
    );
}

#[test]
fn legacy_engine_deserializes_as_local_and_serializes_as_local() {
    let config: SubconsciousConfig =
        serde_json::from_str(r#"{"engine":"medulla","medulla_local":{"serve_entry":"retired"}}"#)
            .unwrap();
    assert_eq!(config.engine, SubconsciousEngine::Local);
    assert_eq!(serde_json::to_string(&config.engine).unwrap(), r#""local""#);
}
