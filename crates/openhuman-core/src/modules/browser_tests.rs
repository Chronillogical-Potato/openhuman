use super::*;

#[test]
fn navigation_uses_shared_allowlist_and_blocks_private_hosts() {
    let mut config = Config::default();
    config.http_request.allowed_domains = vec!["selenium.dev".into()];
    let client = BrowserClient::new(Arc::new(config));
    assert!(client.check_url("https://selenium.dev/").is_ok());
    assert!(client.check_url("https://www.selenium.dev/").is_ok());
    assert!(client.check_url("https://example.com/").is_err());
    assert!(client.check_url("https://evilselenium.dev/").is_err());
    assert!(client.check_url("https://127.0.0.1/").is_err());
    assert!(client.check_url("https://localhost/").is_err());
    assert!(client.check_url("http://selenium.dev/").is_err());
}

#[test]
fn module_origin_list_requires_https_for_allowed_host_tree() {
    if browser_allow_all() {
        return;
    }
    let mut config = Config::default();
    config.http_request.allowed_domains = vec!["selenium.dev".into()];
    let client = BrowserClient::new(Arc::new(config));
    assert_eq!(client.browser_origins(), vec!["https://.selenium.dev"]);
}

#[test]
fn allow_all_starts_denied_and_explicit_urls_bind_one_https_host_tree() {
    let client = BrowserClient::new(Arc::new(Config::default()));
    assert_eq!(client.browser_origins_for_mode(true), vec!["https://."]);
    assert_eq!(
        client
            .explicit_origin_for_mode("https://www.example.com/path", true)
            .unwrap(),
        Some("https://.www.example.com".into())
    );
    assert_eq!(
        client
            .explicit_origin_for_mode("https://other.example/path", true)
            .unwrap(),
        Some("https://.other.example".into())
    );
    for url in [
        "http://example.com",
        "https://localhost",
        "https://127.0.0.1",
        "https://[::1]",
        "https://user@example.com",
    ] {
        assert!(client.explicit_origin_for_mode(url, true).is_err(), "{url}");
    }
}

#[test]
fn published_browser_release_covers_desktop_platforms() {
    let record = registry::find(MODULE_ID).unwrap();
    assert_eq!(record.bus_name, names::INTERFACE);
    assert_eq!(record.object_path, names::OBJECT_PATH);
    assert_eq!(record.version, "0.2.2");
    assert_eq!(
        record.release_url,
        "https://github.com/tinyhumansai/tinybrowser/releases/tag/v0.2.2"
    );
    assert_eq!(record.assets.len(), 16);
    let keys = record
        .assets
        .iter()
        .map(|asset| asset.host_key)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(keys.len(), record.assets.len());
    for asset in record.assets {
        assert!(asset.archive.starts_with("tinybrowser-0.2.2-"));
        assert_eq!(asset.sha256.len(), 64);
    }
}

#[test]
fn fresh_profile_is_the_default() {
    let config = Config::default();
    assert_eq!(config.browser.profile_mode, "fresh");
    assert!(config.browser.profile_path.is_none());
}

#[test]
fn shared_fetch_wildcard_does_not_open_browser_by_default() {
    if browser_allow_all() {
        return;
    }
    let config = Config::default();
    let client = BrowserClient::new(Arc::new(config));
    assert!(client.check_url("https://example.com/").is_err());
}

#[test]
fn blank_session_page_is_reportable_but_not_an_explicit_navigation_target() {
    let client = BrowserClient::new(Arc::new(Config::default()));
    assert!(client.check_returned_url("about:blank").is_ok());
    assert!(client.check_url("about:blank").is_err());
}
