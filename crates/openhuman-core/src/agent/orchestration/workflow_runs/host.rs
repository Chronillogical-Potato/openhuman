//! OpenHuman's thin, policy-owning adapter for the neutral workflow engine.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use tinyagents_harness::CancellationToken;
use tinyagents_orchestration::workflow::{
    OrchestrationError, WorkflowChildRegistration, WorkflowChildRequest, WorkflowChildResult,
    WorkflowExecutor,
};

use crate::agent::orchestration::{
    AgentOrchestrationSession, OrchestrationTaskStatus, SpawnAgentRequest, WaitAgentOptions,
};

/// Host policy has already admitted the parent context before this executor is
/// created.  This adapter only translates one neutral child request into the
/// existing OpenHuman subagent session and retains BUS/progress behaviour there.
#[derive(Clone)]
pub(super) struct OpenHumanWorkflowExecutor {
    session: AgentOrchestrationSession,
    model_override: Option<String>,
}

impl OpenHumanWorkflowExecutor {
    pub(super) fn new(run_id: &str, model_override: Option<String>) -> Self {
        Self {
            session: AgentOrchestrationSession::new(format!("workflow-engine-{run_id}")),
            model_override,
        }
    }
}

#[async_trait]
impl WorkflowExecutor for OpenHumanWorkflowExecutor {
    async fn execute(
        &self,
        request: WorkflowChildRequest,
        cancel: CancellationToken,
        registration: Arc<dyn WorkflowChildRegistration>,
    ) -> Result<WorkflowChildResult, OrchestrationError> {
        if cancel.is_cancelled() {
            return Err(OrchestrationError(
                "workflow cancelled before child spawn".to_owned(),
            ));
        }
        let response = self
            .session
            .spawn_agent(SpawnAgentRequest {
                agent_id: request.agent_id.clone(),
                prompt: request.prompt,
                model: self.model_override.clone(),
                ..Default::default()
            })
            .await
            .map_err(|error| OrchestrationError(error.to_string()))?;

        // This must happen before awaiting the child: stop() can now cancel
        // every real orchestration id while a worker remains in flight.
        registration.register(response.orchestration_id.clone())?;
        if cancel.is_cancelled() {
            self.session.abort_all().await;
            return Err(OrchestrationError(
                "workflow cancelled after child spawn".to_owned(),
            ));
        }
        let wait = self
            .session
            .wait_agents(WaitAgentOptions {
                orchestration_ids: vec![response.orchestration_id.clone()],
                timeout_ms: None,
            })
            .await
            .map_err(|error| OrchestrationError(error.to_string()))?;
        let child = wait.agents.into_iter().next().ok_or_else(|| {
            OrchestrationError("workflow child returned no terminal snapshot".to_owned())
        })?;
        if child.status != OrchestrationTaskStatus::Completed {
            return Err(OrchestrationError(format!(
                "child '{}' (agent '{}') ended {:?}: {}",
                child.orchestration_id,
                child.agent_id,
                child.status,
                child.error.unwrap_or_default()
            )));
        }
        Ok(WorkflowChildResult {
            child_id: response.orchestration_id,
            // Do not coerce to text: the neutral engine faithfully carries
            // host output JSON into downstream prompts and synthesis.
            output: json!({
                "agentId": child.agent_id,
                "summary": child.result_summary,
            }),
        })
    }

    async fn cancel_children(&self, _child_ids: &[String]) {
        // AgentOrchestrationSession is per workflow run, so this aborts exactly
        // its registered children while retaining existing BUS/progress events.
        self.session.abort_all().await;
    }
}
