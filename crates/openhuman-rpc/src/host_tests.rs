use super::*;

#[cfg(feature = "server")]
use openhuman_tinyhumans::embed::seams::DomainGroup;
use openhuman_tinyhumans::embed::{DomainSet, HostKind, ServiceSet};

#[cfg(feature = "server")]
#[test]
fn cli_builder_is_the_cli_preset_with_server_and_http_host() {
    let summary = cli_builder().into_embed().summary();
    assert_eq!(summary.host_kind, HostKind::detect_standalone());
    assert_eq!(summary.domains, Some(DomainSet::full()));
    assert_eq!(summary.services, Some(ServiceSet::desktop()));
    assert!(!summary.fixed_token, "the CLI reads its bearer from env/file");
    assert!(summary.has_server_launcher, "`run`/`serve` need the server");
    assert_eq!(summary.controller_extensions, vec![DomainGroup::Platform]);
}

#[cfg(feature = "server")]
#[test]
fn desktop_builder_carries_bearer_listener_and_services() {
    let options = DesktopOptions {
        host: Some("127.0.0.1".into()),
        port: Some(7799),
        socketio: false,
        rpc_token: Some(Arc::new("launch-bearer".into())),
    };
    let summary = desktop_builder(&options).into_embed().summary();
    assert_eq!(summary.host_kind, HostKind::TauriShell);
    assert_eq!(summary.domains, Some(DomainSet::full()));
    let mut expected = ServiceSet::desktop();
    expected.socketio = false;
    assert_eq!(summary.services, Some(expected));
    assert!(summary.fixed_token);
    assert_eq!(summary.listen_host.as_deref(), Some("127.0.0.1"));
    assert_eq!(summary.listen_port, Some(7799));
    assert!(summary.has_server_launcher);
    assert_eq!(summary.controller_extensions, vec![DomainGroup::Platform]);
    assert!(
        !summary.has_session_store,
        "the server installs the store for the process, not per runtime"
    );
}

#[cfg(feature = "server")]
#[test]
fn desktop_defaults_leave_listener_and_token_to_the_environment() {
    let options = DesktopOptions::default();
    assert!(options.socketio);
    let summary = desktop_builder(&options).into_embed().summary();
    assert!(!summary.fixed_token);
    assert_eq!(summary.listen_host, None);
    assert_eq!(summary.listen_port, None);
    assert_eq!(summary.services, Some(ServiceSet::desktop()));
}

#[cfg(feature = "server")]
#[test]
fn desktop_options_debug_redacts_the_bearer() {
    let options = DesktopOptions {
        rpc_token: Some(Arc::new("secret-bearer".into())),
        ..DesktopOptions::default()
    };
    let rendered = format!("{options:?}");
    assert!(!rendered.contains("secret-bearer"), "{rendered}");
    assert!(rendered.contains("<redacted>"), "{rendered}");
}

#[cfg(feature = "server")]
#[test]
fn desktop_connected_builder_binds_the_transport_and_hosted_controllers() {
    let summary = desktop_builder(&DesktopOptions::default())
        .connect()
        .expect("SDK transport builds offline")
        .summary();
    assert!(summary.has_backend_transport);
    assert!(summary.controller_extensions.contains(&DomainGroup::Hosted));
    assert!(summary.controller_extensions.contains(&DomainGroup::Platform));
}

#[cfg(feature = "session-store")]
#[test]
fn tui_builder_runs_every_domain_without_services_on_the_disk_store() {
    let summary = tui_builder().into_embed().summary();
    assert_eq!(summary.host_kind, HostKind::detect_standalone());
    assert_eq!(summary.domains, Some(DomainSet::full()));
    assert_eq!(summary.services, Some(ServiceSet::none()));
    assert!(summary.has_session_store);
    assert!(!summary.has_server_launcher, "the TUI binds no server");
}
