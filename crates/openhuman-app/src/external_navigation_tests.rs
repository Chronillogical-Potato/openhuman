use super::*;

fn url(s: &str) -> Url {
    Url::parse(s).unwrap()
}

#[test]
fn remote_pages_are_cancelled_and_handed_to_the_os_browser() {
    for href in [
        "https://github.com/tinyhumansai/openhuman",
        "http://example.com/login?token=abc",
        // Loopback that is not the dev server is still a remote page to the webview.
        "http://localhost:8000/",
        "https://tauri.localhost.example.com/",
        // Same host, different origin: a loopback service on another port.
        "http://tauri.localhost:8000/",
    ] {
        assert_eq!(
            navigation_handoff("main", &url(href), None),
            Some(url(href)),
            "{href} must not load in the main webview"
        );
    }
}

#[test]
fn app_origins_stay_in_the_main_webview() {
    for href in [
        "tauri://localhost/#/chat",
        "http://tauri.localhost/#/chat",
        "https://tauri.localhost/#/settings",
        "about:blank",
        "data:text/html,<p>hi</p>",
        "blob:tauri://localhost/5e1c",
    ] {
        assert_eq!(navigation_handoff("main", &url(href), None), None, "{href}");
    }
}

#[test]
fn dev_server_is_an_app_origin_only_when_it_is_the_dev_url() {
    let dev = url("http://localhost:1420");
    assert_eq!(
        navigation_handoff("main", &url("http://localhost:1420/#/chat"), Some(&dev)),
        None
    );
    assert!(navigation_handoff("main", &url("http://localhost:1420/#/chat"), None).is_some());
}

#[test]
fn other_webviews_are_left_to_their_own_handlers() {
    assert_eq!(
        navigation_handoff("ptt-overlay", &url("https://github.com/"), None),
        None
    );
}
