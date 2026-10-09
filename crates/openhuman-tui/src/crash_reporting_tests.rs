use super::*;

#[test]
fn release_tag_uses_the_package_version_without_a_build_sha() {
    if option_env!("OPENHUMAN_BUILD_SHA")
        .unwrap_or("")
        .trim()
        .is_empty()
    {
        assert_eq!(
            build_release_tag(),
            format!("openhuman@{}", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn environment_prefers_app_env_lowercased() {
    assert_eq!(resolve_environment(Some(" Staging ".into())), "staging");
    let fallback = if cfg!(debug_assertions) {
        "development"
    } else {
        "production"
    };
    assert_eq!(resolve_environment(Some("  ".into())), fallback);
}
