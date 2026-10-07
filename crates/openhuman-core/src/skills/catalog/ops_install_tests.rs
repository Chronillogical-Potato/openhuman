use super::*;

use std::sync::atomic::Ordering;

use serde_json::json;

use crate::skills::catalog::test_fixtures::{hermes_item, Fixture};

#[tokio::test]
async fn installs_a_catalog_entry_once_and_announces_it() {
    use crate::core::events::DomainEvent;
    use tinybus::TryRecvError;

    crate::core::bus::init().await.expect("bus init");
    let mut rx = crate::core::bus::BUS
        .get()
        .expect("event bus should be initialized")
        .receiver();

    let fixture = Fixture::start(vec![hermes_item("zz-registry-install", "built-in")]).await;
    let registry = fixture.registry();
    let workspace = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();

    let first = install_from_catalog_in(
        &registry,
        workspace.path(),
        Some(home.path()),
        "zz-registry-install",
    )
    .await
    .expect("install");
    assert_eq!(first.new_skills, ["zz-registry-install"]);
    assert!(first.stdout.contains("Installed to"), "{}", first.stdout);
    assert!(home
        .path()
        .join(".openhuman/skills/zz-registry-install/SKILL.md")
        .exists());

    let mut announced = false;
    loop {
        match rx.try_recv() {
            Ok(DomainEvent::WorkflowsChanged { reason }) if reason == "install" => {
                announced = true;
                break;
            }
            Ok(_) | Err(TryRecvError::Lagged(_)) => continue,
            Err(TryRecvError::Empty) | Err(TryRecvError::Closed) => break,
        }
    }
    assert!(announced, "a catalog install publishes WorkflowsChanged");

    let second = install_from_catalog_in(
        &registry,
        workspace.path(),
        Some(home.path()),
        "zz-registry-install",
    )
    .await
    .expect("a repeat install succeeds");
    assert!(second.new_skills.is_empty());
    assert!(
        second.stdout.contains("already installed"),
        "{}",
        second.stdout
    );
}

#[tokio::test]
async fn a_portal_entry_fails_fast_with_its_source_page() {
    let fixture = Fixture::start(vec![json!({
        "name": "code-audit",
        "description": "x",
        "category": "other",
        "source": "LobeHub",
        "identifier": "lobehub/code-audit",
        "sourceUrl": "https://lobehub.com/agent/code-audit"
    })])
    .await;
    let home = tempfile::tempdir().unwrap();
    let error = install_from_catalog_in(
        &fixture.registry_without_download_base(),
        home.path(),
        Some(home.path()),
        "lobehub/code-audit",
    )
    .await
    .expect_err("no SKILL.md to fetch");
    assert_eq!(error.kind(), Some(RegistryErrorKind::NoDirectDownload));
    let message = error.to_string();
    assert!(
        message.starts_with("SKILL_REGISTRY_NO_DIRECT_DOWNLOAD: "),
        "{message}"
    );
    assert!(
        message.contains("https://lobehub.com/agent/code-audit"),
        "{message}"
    );
}

#[tokio::test]
async fn an_unknown_id_names_real_ids() {
    let fixture = Fixture::start(vec![hermes_item("git-helper", "built-in")]).await;
    let home = tempfile::tempdir().unwrap();
    let error = install_from_catalog_in(
        &fixture.registry(),
        home.path(),
        Some(home.path()),
        "git-helpr",
    )
    .await
    .expect_err("unknown id");
    assert_eq!(error.kind(), Some(RegistryErrorKind::NotFound));
    assert!(error.to_string().contains("git-helper"), "{error}");
}

#[tokio::test]
async fn a_throttled_document_host_reports_rate_limiting() {
    let fixture = Fixture::start(vec![hermes_item("slow-skill", "built-in")]).await;
    fixture.document_status.store(429, Ordering::SeqCst);
    let home = tempfile::tempdir().unwrap();
    let error = install_from_catalog_in(
        &fixture.registry(),
        home.path(),
        Some(home.path()),
        "slow-skill",
    )
    .await
    .expect_err("throttled");
    assert_eq!(error.kind(), Some(RegistryErrorKind::RateLimited));
    let message = error.to_string();
    assert!(
        message.starts_with("SKILL_REGISTRY_RATE_LIMITED: rate limited"),
        "{message}"
    );
    assert!(message.contains("42s"), "{message}");
}
