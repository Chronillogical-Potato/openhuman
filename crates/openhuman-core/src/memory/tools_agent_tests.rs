use super::*;
use crate::core::runtime::{context::CoreContext, ContextOverlay, DomainSet};
use crate::tools::toolpacks::ToolGroups;

fn agent(parent: &Arc<CoreContext>, id: &str) -> Arc<CoreContext> {
    parent.derive_with(
        ContextOverlay::new(
            crate::config::Config::default(),
            DomainSet::kernel(),
            ToolGroups::none(),
        )
        .session_agent(id),
    )
}

fn citation(id: &str) -> TurnCitation {
    TurnCitation {
        id: id.to_string(),
        key: "learning".to_string(),
        namespace: None,
        score: None,
        timestamp: String::new(),
        snippet: String::new(),
    }
}

#[tokio::test]
async fn turn_citations_stay_with_the_agent_that_recorded_them() {
    let root = CoreContext::for_test(DomainSet::full(), None);
    let alpha = agent(&root, "alpha");
    let beta = agent(&root, "beta");
    let thread = "shared-thread";

    CoreContext::scope(Arc::clone(&alpha), async {
        record_pack_citations(thread, vec![citation("alpha-cite")]);
    })
    .await;
    CoreContext::scope(Arc::clone(&beta), async {
        record_pack_citations(thread, vec![citation("beta-cite")]);
    })
    .await;

    let from_beta = CoreContext::scope(beta, async { take_turn_citations(thread) }).await;
    let from_alpha = CoreContext::scope(alpha, async { take_turn_citations(thread) }).await;

    assert_eq!(
        from_alpha.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["alpha-cite"]
    );
    assert_eq!(
        from_beta.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
        ["beta-cite"]
    );
}
