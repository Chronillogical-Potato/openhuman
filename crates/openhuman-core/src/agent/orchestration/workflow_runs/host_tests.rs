use super::*;
use std::collections::BTreeMap;
use tinyagents_orchestration::workflow::WorkflowDefinition;

fn definition(safety_tier: Option<&str>) -> WorkflowDefinition {
    let mut extensions = BTreeMap::new();
    if let Some(safety_tier) = safety_tier {
        extensions.insert("safetyTier".to_owned(), serde_json::json!(safety_tier));
    }
    WorkflowDefinition {
        id: "host-safety".into(),
        name: "host safety".into(),
        description: String::new(),
        phases: Vec::new(),
        default_concurrency: 1,
        max_children: 1,
        extensions,
    }
}

#[test]
fn host_admits_only_explicit_supported_safety_tiers() {
    assert!(matches!(
        admit_workflow(&definition(Some("read_only"))),
        Ok(WorkflowSafetyTier::ReadOnly)
    ));
    assert!(admit_workflow(&definition(None)).is_err());
    assert!(admit_workflow(&definition(Some("full"))).is_err());
}
