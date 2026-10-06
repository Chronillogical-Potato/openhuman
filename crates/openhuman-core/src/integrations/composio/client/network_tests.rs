//! The host's proxy policy for Composio, as handed to the module.

use super::module_transport;
use crate::config::schema::{ProxyConfig, ProxyScope};
use crate::config::{runtime_proxy_config, set_runtime_proxy_config};
use tinyconnectors_bus::{ComposioTlsRoots, ComposioTransportConfig};

const HTTPS_BASE: &str = "https://backend.composio.dev/api/v3";

/// Run `body` with a runtime proxy config installed, then restore the old one.
fn with_proxy<T>(config: ProxyConfig, body: impl FnOnce() -> T) -> T {
    let _lock = crate::config::TEST_ENV_LOCK.blocking_lock();
    let previous = runtime_proxy_config();
    set_runtime_proxy_config(config);
    let result = body();
    set_runtime_proxy_config(previous);
    result
}

fn proxy(all: Option<&str>, http: Option<&str>, https: Option<&str>) -> ProxyConfig {
    ProxyConfig {
        enabled: true,
        all_proxy: all.map(str::to_string),
        http_proxy: http.map(str::to_string),
        https_proxy: https.map(str::to_string),
        // Scoped to Composio so that, while this process-global setting is
        // installed, no other test's loopback request is proxied.
        scope: ProxyScope::Services,
        services: vec!["tool.composio".into()],
        ..ProxyConfig::default()
    }
}

#[test]
fn no_proxy_configured_is_the_modules_default() {
    let config = with_proxy(ProxyConfig::default(), || module_transport(HTTPS_BASE));
    // On Windows the platform store is always requested; elsewhere the policy
    // is empty and nothing is sent at all.
    if cfg!(target_os = "windows") {
        assert_eq!(config.unwrap().tls_roots, ComposioTlsRoots::Platform);
    } else {
        assert_eq!(config, None);
    }
}

#[test]
fn an_https_destination_takes_all_proxy_first_then_https_proxy() {
    let all = with_proxy(
        proxy(Some("http://all:1"), Some("http://h:2"), Some("http://s:3")),
        || module_transport(HTTPS_BASE),
    );
    assert_eq!(all.unwrap().proxy_url.as_deref(), Some("http://all:1"));

    let https = with_proxy(proxy(None, Some("http://h:2"), Some("http://s:3")), || {
        module_transport(HTTPS_BASE)
    });
    assert_eq!(https.unwrap().proxy_url.as_deref(), Some("http://s:3"));

    let http_only = with_proxy(proxy(None, Some("http://h:2"), None), || {
        module_transport(HTTPS_BASE)
    });
    assert!(http_only.and_then(|c| c.proxy_url).is_none());
}

#[test]
fn a_plain_http_destination_takes_http_proxy() {
    let config = with_proxy(proxy(None, Some("http://h:2"), Some("http://s:3")), || {
        module_transport("http://127.0.0.1:9/api/v3")
    });
    assert_eq!(config.unwrap().proxy_url.as_deref(), Some("http://h:2"));
}

#[test]
fn no_proxy_entries_travel_with_the_proxy_only() {
    let mut with = proxy(Some("http://all:1"), None, None);
    with.no_proxy = vec!["localhost, .internal".into(), "10.0.0.0/8".into()];
    let config = with_proxy(with, || module_transport(HTTPS_BASE)).unwrap();
    assert_eq!(
        config.no_proxy,
        [".internal", "10.0.0.0/8", "localhost"],
        "normalised: split, trimmed, sorted"
    );

    // No proxy URL applies, so there is nothing to exempt a host from.
    let mut without = proxy(None, Some("http://h:2"), None);
    without.no_proxy = vec!["localhost".into()];
    let config = with_proxy(without, || module_transport(HTTPS_BASE));
    assert!(config.is_none_or(|c| c.no_proxy.is_empty()));
}

#[test]
fn a_scope_that_leaves_composio_out_sends_no_proxy() {
    let mut scoped = proxy(Some("http://all:1"), None, None);
    scoped.scope = ProxyScope::Services;
    scoped.services = vec!["provider.openai".into()];
    let config = with_proxy(scoped, || module_transport(HTTPS_BASE));
    assert!(config.and_then(|c| c.proxy_url).is_none());

    let mut covered = proxy(Some("http://all:1"), None, None);
    covered.scope = ProxyScope::Services;
    covered.services = vec!["tool.*".into()];
    let config = with_proxy(covered, || module_transport(HTTPS_BASE));
    assert_eq!(
        config.and_then(|c| c.proxy_url).as_deref(),
        Some("http://all:1")
    );

    let mut environment = proxy(Some("http://all:1"), None, None);
    environment.scope = ProxyScope::Environment;
    let config = with_proxy(environment, || module_transport(HTTPS_BASE));
    assert!(config.and_then(|c| c.proxy_url).is_none());
}

#[test]
fn an_unusable_proxy_url_is_ignored_like_the_hosts_own_clients_do() {
    let config = with_proxy(proxy(Some("not a url"), None, Some("http://s:3")), || {
        module_transport(HTTPS_BASE)
    });
    assert_eq!(
        config.and_then(|c| c.proxy_url).as_deref(),
        Some("http://s:3"),
        "the next usable URL is used instead"
    );

    let none = with_proxy(proxy(Some("   "), None, None), || {
        module_transport(HTTPS_BASE)
    });
    assert!(none.and_then(|c| c.proxy_url).is_none());
}

#[test]
fn the_default_value_is_never_sent() {
    // Equal to the module's own default means "send nothing".
    let config = with_proxy(ProxyConfig::default(), || module_transport(HTTPS_BASE));
    assert_ne!(config, Some(ComposioTransportConfig::default()));
}
