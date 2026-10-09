use super::*;
use crate::core::runtime::{ContextOverlay, CoreContext, DomainSet};

#[tokio::test]
async fn a_job_completed_as_an_agent_is_noted_once() {
    let agent = CoreContext::for_test(DomainSet::full(), None).derive_with(
        ContextOverlay::new(
            crate::config::Config::default(),
            DomainSet::full(),
            Default::default(),
        )
        .session_agent("completion-owner-agent"),
    );
    CoreContext::scope(agent, async { note("completion-owner-job") }).await;
    assert_eq!(
        take("completion-owner-job").as_deref(),
        Some("completion-owner-agent")
    );
    assert_eq!(take("completion-owner-job"), None, "taken once");
}

#[test]
fn a_job_completed_without_an_agent_is_not_noted() {
    note("completion-owner-local");
    assert_eq!(take("completion-owner-local"), None);
}
