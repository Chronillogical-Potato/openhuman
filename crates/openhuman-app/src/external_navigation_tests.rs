use super::*;

#[test]
fn remote_navigation_is_external_but_app_origins_are_preserved() {
    let dev = Url::parse("http://localhost:1420").unwrap();
    for href in [
        "https://github.com/example/repo/pull/1",
        "http://localhost:8000",
        "https://tauri.localhost.example.com",
    ] {
        assert!(
            is_external(&Url::parse(href).unwrap(), Some(&dev)),
            "{href}"
        );
    }
    for href in [
        "tauri://localhost/#/chat",
        "http://tauri.localhost/#/chat",
        "https://tauri.localhost/#/chat",
        "http://localhost:1420/#/chat",
    ] {
        assert!(
            !is_external(&Url::parse(href).unwrap(), Some(&dev)),
            "{href}"
        );
    }
    assert!(is_external(&dev, None));
}
