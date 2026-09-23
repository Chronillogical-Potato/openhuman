use super::*;
use crate::{
    config::Config,
    flows::{
        agents::workflow_builder::builder_prompt::{BuildMode, BuilderRequest},
        ops::{
            builder::{backend_repair_message, is_backend_or_infrastructure_failure},
            flows_build_with_extra_hidden_tools,
        },
    },
};

#[test]
fn classifies_backend_failures_without_classifying_graph_timeouts() {
    assert!(is_backend_or_infrastructure_failure(
        "File upload failed: Backend returned 500 Internal Server Error"
    ));
    assert!(is_backend_or_infrastructure_failure(
        "connection timed out while calling file storage"
    ));
    assert!(!is_backend_or_infrastructure_failure(
        "agent node fetch_profile timed out after 30 seconds"
    ));
    assert!(!is_backend_or_infrastructure_failure(
        "required argument resolved null: nodes.get_link.item.json.url"
    ));
}

#[test]
fn backend_repair_message_explains_why_the_graph_was_preserved() {
    let message = backend_repair_message("HTTP 503 from file storage").unwrap();

    assert!(message.contains("workflow was not changed"));
    assert!(message.contains("HTTP 503 from file storage"));
    assert!(
        backend_repair_message("agent node fetch_profile timed out after 30 seconds").is_none()
    );
}

#[tokio::test]
async fn repair_backend_failure_returns_no_proposal_before_starting_the_builder() {
    let req = BuilderRequest {
        mode: BuildMode::Repair,
        instruction: "repair the failed workflow".to_string(),
        graph: Some(json!({"nodes": [], "edges": []})),
        flow_id: Some("flow-1".to_string()),
        run_id: Some("run-1".to_string()),
        error: Some("Backend returned 503 Service Unavailable".to_string()),
        failing_node_ids: vec!["fetch".to_string()],
    };

    let outcome = flows_build_with_extra_hidden_tools(&Config::default(), req, None, &[])
        .await
        .expect("backend failure should short-circuit before the builder starts");

    assert_eq!(outcome.value["proposal"], Value::Null);
    assert_eq!(outcome.value["error"], Value::Null);
    assert!(
        outcome.value["assistant_text"]
            .as_str()
            .expect("assistant text")
            .contains("workflow was not changed")
    );
}
