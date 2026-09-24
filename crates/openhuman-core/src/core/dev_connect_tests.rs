use super::*;

#[test]
fn enabled_in_debug_builds_or_with_opt_in() {
    assert!(dev_connect_enabled_with(true, None));
    assert!(dev_connect_enabled_with(false, Some("1")));
    assert!(dev_connect_enabled_with(false, Some(" true ")));
    assert!(!dev_connect_enabled_with(false, None));
    assert!(!dev_connect_enabled_with(false, Some("0")));
    assert!(!dev_connect_enabled_with(false, Some("")));
}

#[test]
fn app_origin_accepts_only_bare_http_loopback() {
    assert_eq!(
        normalize_app_origin("http://localhost:1420").as_deref(),
        Some("http://localhost:1420")
    );
    assert_eq!(
        normalize_app_origin("http://127.0.0.1:1430/").as_deref(),
        Some("http://127.0.0.1:1430")
    );
    assert_eq!(
        normalize_app_origin("http://[::1]:1420").as_deref(),
        Some("http://[::1]:1420")
    );
    for bad in [
        "https://localhost:1420",
        "http://evil.example:1420",
        "http://localhost.evil.example",
        "http://user:pw@localhost:1420",
        "http://localhost:1420/other",
        "http://localhost:1420/?x=1",
        "javascript:alert(1)",
        "",
    ] {
        assert_eq!(normalize_app_origin(bad), None, "{bad} must be rejected");
    }
}

#[test]
fn rpc_url_comes_from_a_loopback_host_header() {
    assert_eq!(
        rpc_url_from_host("127.0.0.1:7788").as_deref(),
        Some("http://127.0.0.1:7788/rpc")
    );
    assert_eq!(
        rpc_url_from_host("localhost:7790").as_deref(),
        Some("http://localhost:7790/rpc")
    );
    assert_eq!(rpc_url_from_host("192.168.1.5:7788"), None);
    assert_eq!(rpc_url_from_host("evil.example:7788"), None);
    // No explicit port means we cannot say where the core listens.
    assert_eq!(rpc_url_from_host("127.0.0.1"), None);
}

#[test]
fn cross_site_navigations_are_refused() {
    assert!(is_direct_navigation(None));
    assert!(is_direct_navigation(Some("none")));
    assert!(is_direct_navigation(Some("same-origin")));
    assert!(!is_direct_navigation(Some("cross-site")));
    assert!(!is_direct_navigation(Some("same-site")));
}

#[test]
fn redirect_carries_credentials_in_the_fragment_only() {
    let target = build_redirect("http://localhost:1420", "http://127.0.0.1:7788/rpc", "a+b/c");
    let (before_hash, fragment) = target.split_once('#').expect("fragment");
    assert_eq!(before_hash, "http://localhost:1420/__dev-connect");
    assert!(!before_hash.contains("token"));
    assert_eq!(
        fragment,
        "rpcUrl=http%3A%2F%2F127.0.0.1%3A7788%2Frpc&token=a%2Bb%2Fc"
    );
}
