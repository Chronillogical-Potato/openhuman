use super::*;
use crate::config::schema::{DelegateAgentConfig, ModelRouteConfig, TeamModelConfig};

fn delegate(model: &str) -> DelegateAgentConfig {
    DelegateAgentConfig {
        model: model.to_string(),
        system_prompt: None,
        temperature: None,
        max_depth: 3,
    }
}

#[test]
fn rewrites_every_tier_slug_to_the_managed_default() {
    let mut config = Config::default();
    config.default_model = Some("chat-v1".to_string());
    config.orchestrator.model = Some("reasoning-v1".to_string());
    config.teams.insert(
        "research".to_string(),
        TeamModelConfig {
            lead_model: Some("agentic-v1".to_string()),
            agent_model: Some("burst-v1".to_string()),
        },
    );
    config
        .agents
        .insert("coder".to_string(), delegate("coding-v1"));
    config.model_routes.push(ModelRouteConfig {
        hint: "summarization".to_string(),
        model: "summarization-v1".to_string(),
    });

    let stats = run(&mut config).expect("migration runs");

    assert_eq!(stats.rewritten, 6);
    assert_eq!(config.default_model.as_deref(), Some(MODEL_MANAGED_DEFAULT));
    assert_eq!(
        config.orchestrator.model.as_deref(),
        Some(MODEL_MANAGED_DEFAULT)
    );
    assert_eq!(
        config.teams["research"].lead_model.as_deref(),
        Some(MODEL_MANAGED_DEFAULT)
    );
    assert_eq!(
        config.teams["research"].agent_model.as_deref(),
        Some(MODEL_MANAGED_DEFAULT)
    );
    assert_eq!(config.agents["coder"].model, MODEL_MANAGED_DEFAULT);
    assert_eq!(config.model_routes[0].model, MODEL_MANAGED_DEFAULT);
    // The route's hint key is the role and stays.
    assert_eq!(config.model_routes[0].hint, "summarization");
}

#[test]
fn leaves_concrete_ids_hints_and_empty_values_alone() {
    let mut config = Config::default();
    config.default_model = Some("openrouter/deepseek/deepseek-v4-pro".to_string());
    config.orchestrator.model = Some("hint:reasoning".to_string());
    config
        .agents
        .insert("byok".to_string(), delegate("gpt-4o"));

    let stats = run(&mut config).expect("migration runs");

    assert_eq!(stats, MigrationStats::default());
    assert_eq!(
        config.default_model.as_deref(),
        Some("openrouter/deepseek/deepseek-v4-pro")
    );
    assert_eq!(config.orchestrator.model.as_deref(), Some("hint:reasoning"));
    assert_eq!(config.agents["byok"].model, "gpt-4o");
}

#[test]
fn is_idempotent() {
    let mut config = Config::default();
    config.default_model = Some("reasoning-quick-v1".to_string());
    run(&mut config).expect("first run");
    let stats = run(&mut config).expect("second run");
    assert_eq!(stats.rewritten, 0);
    assert_eq!(config.default_model.as_deref(), Some(MODEL_MANAGED_DEFAULT));
}
