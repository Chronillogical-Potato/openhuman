use super::*;

fn spec(name: &str) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("{name} description"),
        parameters: serde_json::json!({
            "type": "object",
            "properties": { "to": { "type": "string" } }
        }),
    }
}

fn integration(
    toolkit: &str,
    connected: bool,
    gated_tools: Vec<crate::agent::prompts::GatedIntegrationTool>,
) -> crate::agent::prompts::ConnectedIntegration {
    crate::agent::prompts::ConnectedIntegration {
        toolkit: toolkit.into(),
        description: String::new(),
        tools: Vec::new(),
        gated_tools,
        connected,
        connections: Vec::new(),
        non_active_status: None,
    }
}

#[test]
fn only_composio_slugs_count_as_integration_actions() {
    assert!(is_integration_action_name("GMAIL_SEND_EMAIL"));
    assert!(is_integration_action_name("GOOGLECALENDAR_CREATE_EVENT"));
    assert!(!is_integration_action_name("memory_recall"));
    assert!(!is_integration_action_name("tool_search"));
    assert!(!is_integration_action_name("GMAIL"));
    assert!(!is_integration_action_name("_SEND"));
}

#[test]
fn recorded_actions_are_filtered_from_a_mixed_declaration_set() {
    let recorded = vec![
        spec("web_fetch"),
        spec("GMAIL_SEND_EMAIL"),
        spec("research"),
    ];
    let actions = recorded_integration_actions(&recorded);
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].name, "GMAIL_SEND_EMAIL");
}

#[test]
fn a_restart_with_no_integrations_rebuilds_the_recorded_actions_as_deferred() {
    let recorded = vec![spec("GMAIL_SEND_EMAIL"), spec("GMAIL_FETCH_EMAILS")];
    let rebuilt = rehydrate_integration_actions(&recorded, &[], &[], false);

    let names: Vec<&str> = rebuilt.iter().map(|tool| tool.name()).collect();
    assert_eq!(names, vec!["GMAIL_SEND_EMAIL", "GMAIL_FETCH_EMAILS"]);
    for tool in &rebuilt {
        assert_eq!(tool.exposure(), tinytools::ToolExposure::Deferred);
        assert_eq!(tool.family(), Some("gmail"));
    }
    // The declaration round-trips: same description, and the schema keeps the
    // recorded properties (the tool only adds `connection_id` when absent).
    let schema = rebuilt[0].parameters_schema();
    assert_eq!(rebuilt[0].description(), "GMAIL_SEND_EMAIL description");
    assert!(schema["properties"]["to"].is_object());
}

#[test]
fn a_live_action_is_not_rebuilt_from_the_record() {
    let recorded = vec![spec("GMAIL_SEND_EMAIL"), spec("SLACK_SEND_MESSAGE")];
    let live: Vec<Box<dyn Tool>> =
        rehydrate_integration_actions(&[spec("GMAIL_SEND_EMAIL")], &[], &[], false);
    let rebuilt = rehydrate_integration_actions(&recorded, &live, &[], false);
    let names: Vec<&str> = rebuilt.iter().map(|tool| tool.name()).collect();
    assert_eq!(names, vec!["SLACK_SEND_MESSAGE"]);
}

#[test]
fn a_rebuilt_declaration_is_byte_identical_to_the_recorded_one() {
    let recorded = vec![spec("GMAIL_SEND_EMAIL")];
    let first = rehydrate_integration_actions(&recorded, &[], &[], false);
    let second = rehydrate_integration_actions(&recorded, &[], &[], false);
    let mut rebuilt = first[0].spec();
    rebuilt
        .parameters
        .get_mut("properties")
        .and_then(serde_json::Value::as_object_mut)
        .expect("rebuilt properties")
        .remove("connection_id");
    assert_eq!(
        serde_json::to_string(&rebuilt).unwrap(),
        serde_json::to_string(&recorded[0]).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&first[0].spec()).unwrap(),
        serde_json::to_string(&second[0].spec()).unwrap()
    );
}

#[test]
fn authoritative_integrations_do_not_restore_revoked_or_gated_actions() {
    let recorded = vec![spec("GMAIL_SEND_EMAIL"), spec("SLACK_SEND_MESSAGE")];
    let integrations = vec![
        integration(
            "gmail",
            true,
            vec![crate::agent::prompts::GatedIntegrationTool {
                name: "GMAIL_SEND_EMAIL".into(),
                description: String::new(),
                required_scope: "write".into(),
                unlock_paths: Vec::new(),
            }],
        ),
        integration("slack", false, Vec::new()),
    ];

    let rebuilt = rehydrate_integration_actions(&recorded, &[], &integrations, true);
    assert!(rebuilt.is_empty());
}
