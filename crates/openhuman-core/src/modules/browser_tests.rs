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
fn non_global_ip_literals_stay_blocked() {
    for host in [
        "198.18.0.1",
        "240.0.0.1",
        "255.255.255.255",
        "198.51.100.1",
        "0.1.2.3",
        "[2001:db8::1]",
    ] {
        assert!(
            private_or_local(host.trim_matches(['[', ']'])),
            "{host} must be blocked"
        );
    }
    assert!(!private_or_local("93.184.216.34"));
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
fn browser_members_share_the_tinycomputer_record() {
    let record = crate::modules::registry::find(MODULE_ID).unwrap();
    assert_eq!(MODULE_ID, "tinycomputer");
    assert_eq!(record.bus_name, names::INTERFACE);
    assert_eq!(record.object_path, names::OBJECT_PATH);
    assert_eq!(names::INTERFACE, tinycomputer_bus::names::INTERFACE);
    for member in [
        names::methods::OPEN_SESSION,
        names::methods::NAVIGATE,
        names::methods::SNAPSHOT,
        names::methods::PERFORM,
        names::methods::READ_PAGE,
        names::methods::LIST_DOWNLOADS,
        names::methods::WAIT_DOWNLOAD,
        names::methods::CLOSE_SESSION,
    ] {
        assert!(
            tinycomputer_bus::names::METHODS.contains(&member),
            "{member} is not served"
        );
    }
}

#[test]
fn failed_reply_maps_to_the_browser_error_name() {
    let response: DesktopResponse = serde_json::from_value(serde_json::json!({
        "version": "2.5",
        "ok": false,
        "command": "browser-navigate",
        "error": {
            "code": "POLICY_DENIED",
            "message": "origin is not allowed",
            "details": {"name": "ai.tinyhumans.tinycomputer.Browser.Error.BlockedByPolicy"}
        }
    }))
    .unwrap();
    match BrowserCallError::from_response(&response) {
        BrowserCallError::Bus { name, message } => {
            assert_eq!(name, "BlockedByPolicy");
            assert_eq!(message, "origin is not allowed");
        }
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn failed_reply_without_a_name_falls_back_to_the_code() {
    let response: DesktopResponse = serde_json::from_value(serde_json::json!({
        "version": "2.5",
        "ok": false,
        "command": "browser-open-session",
        "error": {"code": "BROWSER_UNAVAILABLE", "message": "no chrome"}
    }))
    .unwrap();
    assert!(matches!(
        BrowserCallError::from_response(&response),
        BrowserCallError::Bus { name, .. } if name == "BROWSER_UNAVAILABLE"
    ));
}

#[test]
fn a_missing_chrome_tells_the_agent_where_the_user_sets_its_path() {
    let response: DesktopResponse = serde_json::from_value(serde_json::json!({
        "version": "2.5",
        "ok": false,
        "command": "browser-open-session",
        "error": {
            "code": "BROWSER_UNAVAILABLE",
            "message": "browser unavailable: Chrome not found. Checked: - System Chrome installations",
            "details": {"name": "ai.tinyhumans.tinycomputer.Browser.Error.BrowserUnavailable"}
        }
    }))
    .unwrap();
    let error = BrowserCallError::from_response(&response).to_string();
    assert!(error.starts_with("BrowserUnavailable: browser unavailable: Chrome not found"));
    assert!(error.contains("Chrome path"), "{error}");
    assert!(error.contains("Do not install"), "{error}");
    assert!(chrome_not_found("CHROME NOT FOUND"));
    assert!(!chrome_not_found("Chrome launch timed out"));
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

/// Drives real URLs through `check_url` (parsing, host extraction, policy),
/// so the non-global block is proven on the path the browser tool uses, not
/// only on the helper.
#[test]
fn navigation_rejects_non_global_ip_literal_urls() {
    let client = BrowserClient::new(Arc::new(Config::default()));
    for url in [
        "https://198.18.0.1/",
        "https://240.0.0.1/",
        "https://0.1.2.3/",
        "https://198.51.100.1/path",
        "https://203.0.113.9/",
        "https://[2001:db8::1]/",
    ] {
        let error = client.check_url(url).expect_err(url).to_string();
        assert!(error.contains("is blocked"), "{url}: {error}");
    }
}
