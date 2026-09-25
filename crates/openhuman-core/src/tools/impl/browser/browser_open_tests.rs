use super::*;

#[test]
fn simple_open_still_obeys_shared_website_policy() {
    let mut config = crate::config::Config::default();
    config.http_request.allowed_domains = vec!["selenium.dev".into()];
    let client = Arc::new(BrowserClient::new(Arc::new(config)));
    let tool = BrowserOpenTool::new(Arc::new(SecurityPolicy::default()), client);
    assert_eq!(tool.exposure(), tinytools::ToolExposure::Deferred);
    assert!(tool
        .validate_url("https://www.selenium.dev/selenium/web/web-form.html")
        .is_ok());
    assert!(tool.validate_url("https://example.com").is_err());
    assert!(tool.validate_url("http://selenium.dev").is_err());
    assert!(tool.validate_url("https://localhost/").is_err());
}

#[test]
fn simple_open_rejects_private_and_plain_http_destinations() {
    let client = Arc::new(BrowserClient::new(Arc::new(
        crate::config::Config::default(),
    )));
    let tool = BrowserOpenTool::new(Arc::new(SecurityPolicy::default()), client);
    for url in [
        "http://example.com",
        "https://localhost",
        "https://127.0.0.1",
        "https://[::1]",
    ] {
        assert!(tool.validate_url(url).is_err(), "{url}");
    }
}
