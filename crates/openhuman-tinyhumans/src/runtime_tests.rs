//! The builder is the configuration path: it must hand the embed builder the
//! SDK transport, the hosted controllers and the Jev ranker, set the product
//! identity, and forward every preset and knob unchanged.

use super::*;
use openhuman_embed::seams::DomainGroup;
use openhuman_embed::{BackendTransport, BuilderSummary};

fn connected(builder: RuntimeBuilder) -> BuilderSummary {
    builder.connect().expect("connect").summary()
}

#[test]
fn connect_wires_transport_hosted_controllers_and_ranker() {
    let summary = connected(RuntimeBuilder::new());
    assert!(summary.has_backend_transport);
    assert_eq!(summary.controller_extensions, vec![DomainGroup::Hosted]);
    #[cfg(feature = "jev")]
    assert_eq!(
        summary.tool_ranker.as_deref(),
        Some(ToolRanker::kind(&crate::jev::TinyHumansJevRanker::new()))
    );
    #[cfg(not(feature = "jev"))]
    assert!(summary.tool_ranker.is_none());
    // The transport is also the process global, as `install` leaves it.
    assert!(openhuman_embed::installed_backend_transport().is_some());
}

#[test]
fn into_embed_stays_unconnected() {
    let summary = RuntimeBuilder::new().into_embed().summary();
    assert!(!summary.has_backend_transport);
    assert!(summary.controller_extensions.is_empty());
    assert!(summary.tool_ranker.is_none());
}

#[test]
fn hosted_controllers_and_jev_can_be_turned_off() {
    let summary = connected(
        RuntimeBuilder::new()
            .hosted_controllers(false)
            .jev_ranker(false),
    );
    assert!(summary.has_backend_transport);
    assert!(summary.controller_extensions.is_empty());
    assert!(summary.tool_ranker.is_none());
}

#[test]
fn product_identity_is_set_on_connect() {
    let _guard = crate::backend::product::product_identity_test_lock();
    let identity = crate::ProductIdentity::new("opencompany").unwrap();
    let builder = RuntimeBuilder::new().product_identity(identity.clone());
    // Not applied until the builder connects.
    assert_ne!(crate::product_identity(), identity);
    let wired = wiring(&builder.install).expect("wiring");
    assert_eq!(crate::product_identity(), identity);
    assert_eq!(wired.transport.product_identity(), "opencompany");

    // Restore the default identity and a transport carrying it.
    let restored = wiring(
        &InstallOptions::default()
            .hosted_controllers(false)
            .tool_ranker(false)
            .product_identity(crate::ProductIdentity::default()),
    )
    .expect("restore");
    assert_eq!(
        restored.transport.product_identity(),
        crate::backend::DEFAULT_PRODUCT_IDENTITY
    );
}

#[test]
fn presets_forward_to_the_embed_presets() {
    let pairs = [
        (
            RuntimeBuilder::library(),
            openhuman_embed::RuntimeBuilder::library(),
        ),
        (
            RuntimeBuilder::desktop(),
            openhuman_embed::RuntimeBuilder::desktop(),
        ),
        (
            RuntimeBuilder::cli(),
            openhuman_embed::RuntimeBuilder::cli(),
        ),
        (
            RuntimeBuilder::tui(),
            openhuman_embed::RuntimeBuilder::tui(),
        ),
    ];
    for (ours, theirs) in pairs {
        let (ours, theirs) = (ours.into_embed().summary(), theirs.summary());
        assert_eq!(ours.host_kind, theirs.host_kind);
        assert_eq!(ours.domains, theirs.domains);
        assert_eq!(ours.services, theirs.services);
        assert_eq!(ours.config_source, theirs.config_source);
        assert_eq!(
            format!("{:?}", ours.workspace),
            format!("{:?}", theirs.workspace)
        );
    }
}

#[test]
fn knobs_and_seams_forward_unchanged() {
    struct Ranker;
    #[async_trait::async_trait]
    impl ToolRanker for Ranker {
        fn kind(&self) -> &'static str {
            "test-ranker"
        }
        async fn rank(
            &self,
            _intent: &str,
            _context: &openhuman_embed::seams::RankContext,
            _candidates: &[openhuman_embed::seams::RankCandidate],
            _limit: usize,
        ) -> Result<Vec<openhuman_embed::seams::RankHit>, openhuman_embed::seams::RankError>
        {
            Ok(Vec::new())
        }
    }

    let summary = connected(
        RuntimeBuilder::desktop()
            .token(TokenSource::Fixed(Arc::new("bearer".to_string())))
            .listen("127.0.0.1", 7791)
            .action_dir("/srv/work")
            .workspace_dir("/srv/state")
            .session_store(Arc::new(openhuman_embed::InMemorySessionStores::new()))
            .tool_ranker(Arc::new(Ranker)),
    );
    assert!(summary.fixed_token);
    assert_eq!(summary.listen_host.as_deref(), Some("127.0.0.1"));
    assert_eq!(summary.listen_port, Some(7791));
    assert_eq!(summary.action_dir, Some(PathBuf::from("/srv/work")));
    assert_eq!(summary.workspace_dir, Some(PathBuf::from("/srv/state")));
    assert!(summary.has_session_store);
    // A host ranker replaces the Jev one rather than being overwritten by it.
    assert_eq!(summary.tool_ranker.as_deref(), Some("test-ranker"));
    assert_eq!(summary.controller_extensions, vec![DomainGroup::Hosted]);
}
