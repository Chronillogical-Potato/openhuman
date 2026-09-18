use super::*;

#[test]
fn rewrites_legacy_engine_to_local() {
    let mut config = Config::default();
    config.subconscious.engine = SubconsciousEngine::Medulla;
    config.subconscious.medulla_local.serve_entry = "/legacy/serve.js".into();

    assert!(run(&mut config).unwrap());
    assert_eq!(config.subconscious.engine, SubconsciousEngine::Local);
    assert_eq!(
        config.subconscious.medulla_local.serve_entry,
        "/legacy/serve.js"
    );
}

#[test]
fn leaves_local_engine_unchanged() {
    let mut config = Config::default();

    assert!(!run(&mut config).unwrap());
    assert_eq!(config.subconscious.engine, SubconsciousEngine::Local);
}
