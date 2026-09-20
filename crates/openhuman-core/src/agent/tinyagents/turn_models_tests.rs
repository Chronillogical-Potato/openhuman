use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tinyagents_harness::host::{ModelResolveRequest, ModelResolver};
use tinyinference_llm::message::ContentBlock;
use tinyinference_llm::model::{ChatModel, ModelRequest, ModelResponse};

use super::TurnModelResolver;

/// A stub that answers with its own name so a test can tell which model the
/// resolver handed back.
struct NamedModel(&'static str);

#[async_trait]
impl ChatModel<()> for NamedModel {
    async fn invoke(
        &self,
        _state: &(),
        _request: ModelRequest,
    ) -> tinyinference_llm::Result<ModelResponse> {
        Ok(ModelResponse::assistant(self.0))
    }
}

async fn name_of(model: &Arc<dyn ChatModel<()>>) -> String {
    let response = model
        .invoke(&(), ModelRequest::default())
        .await
        .expect("stub model never fails");
    response
        .message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(text) => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

fn resolver() -> TurnModelResolver {
    let primary: Arc<dyn ChatModel<()>> =
        Arc::new(NamedModel("openrouter/deepseek/deepseek-v4.1-flash"));
    let mut routes: HashMap<String, Arc<dyn ChatModel<()>>> = HashMap::new();
    routes.insert("hint:coding".to_string(), Arc::new(NamedModel("hint:coding")));
    routes.insert("hint:burst".to_string(), Arc::new(NamedModel("hint:burst")));
    TurnModelResolver::new(primary, routes)
}

/// Regression for the orchestrator's `hint = "coding"` overriding the user's
/// UI model pick: the turn lead must get the selected primary even when its
/// definition pin names a built tier route.
#[tokio::test]
async fn lead_keeps_selected_primary_over_definition_pin() {
    let request = ModelResolveRequest::new("orchestrator")
        .as_team_lead()
        .with_model_pin("hint:coding");
    let model = resolver().resolve(&request).await.expect("resolves");
    assert_eq!(
        name_of(&model).await,
        "openrouter/deepseek/deepseek-v4.1-flash"
    );
}

#[tokio::test]
async fn lead_without_pin_gets_primary() {
    let request = ModelResolveRequest::new("orchestrator").as_team_lead();
    let model = resolver().resolve(&request).await.expect("resolves");
    assert_eq!(
        name_of(&model).await,
        "openrouter/deepseek/deepseek-v4.1-flash"
    );
}

#[tokio::test]
async fn subagent_pin_resolves_to_its_tier_route() {
    let request = ModelResolveRequest::new("integrations_agent").with_model_pin("hint:burst");
    let model = resolver().resolve(&request).await.expect("resolves");
    assert_eq!(name_of(&model).await, "hint:burst");
}

#[tokio::test]
async fn subagent_pin_without_route_falls_back_to_primary() {
    let request = ModelResolveRequest::new("worker").with_model_pin("hint:vision");
    let model = resolver().resolve(&request).await.expect("resolves");
    assert_eq!(
        name_of(&model).await,
        "openrouter/deepseek/deepseek-v4.1-flash"
    );
}
