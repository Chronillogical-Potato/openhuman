//! OpenHuman's workflow catalog and ledger read adapters.
//!
//! Definitions, validation, scheduling and fan-out live in
//! `tinyagents-orchestration`. This module retains only product catalog
//! selection, live agent lookup and the configured persistence root.

use std::collections::BTreeMap;

use anyhow::Result;
use serde_json::json;
use tinyagents_orchestration::workflow::{
    validate_agents, validate_structure, DefinitionError, WorkflowDefinition,
    WorkflowDefinitionListResponse, WorkflowPhase,
};
use tinyagents_session::run_ledger::{
    get_workflow_run, list_workflow_runs, WorkflowRun, WorkflowRunListRequest,
    WorkflowRunListResponse,
};

use crate::agent::harness::definition::AgentDefinitionRegistry;
use crate::config::Config;

pub const PARALLEL_RESEARCH_ID: &str = "parallel_research_cross_check";

/// Product-selected workflow catalog. `safetyTier` remains opaque data here;
/// the host's admission adapter remains responsible for policy enforcement.
pub fn builtin_definitions() -> Vec<WorkflowDefinition> {
    vec![WorkflowDefinition {
        id: PARALLEL_RESEARCH_ID.to_owned(),
        name: "Parallel research with cross-checking".to_owned(),
        description: "Decompose a question into angles, research them in parallel, cross-check the claims with a critic, then synthesize a cited report. Read-only.".to_owned(),
        phases: vec![
            WorkflowPhase { name: "decompose".into(), description: "Break the question into independent research angles.".into(), agent_ids: vec!["planner".into()], depends_on: vec![] },
            WorkflowPhase { name: "research".into(), description: "Research each angle in parallel.".into(), agent_ids: vec!["researcher".into(), "researcher".into()], depends_on: vec!["decompose".into()] },
            WorkflowPhase { name: "cross_check".into(), description: "Adversarially cross-check the gathered claims.".into(), agent_ids: vec!["critic".into()], depends_on: vec!["research".into()] },
            WorkflowPhase { name: "synthesize".into(), description: "Synthesize a single cited report.".into(), agent_ids: vec!["summarizer".into()], depends_on: vec!["cross_check".into()] },
        ],
        default_concurrency: 2,
        max_children: 8,
        extensions: BTreeMap::from([("safetyTier".to_owned(), json!("read_only"))]),
    }]
}

pub fn definition_by_id(id: &str) -> Option<WorkflowDefinition> {
    builtin_definitions()
        .into_iter()
        .find(|definition| definition.id == id)
}

pub fn list_definitions() -> WorkflowDefinitionListResponse {
    let definitions = builtin_definitions();
    WorkflowDefinitionListResponse {
        count: definitions.len(),
        definitions,
    }
}

pub fn validate_definition(definition: &WorkflowDefinition) -> Vec<DefinitionError> {
    let mut errors = validate_structure(definition);
    if let Some(registry) = AgentDefinitionRegistry::global() {
        errors.extend(validate_agents(definition, |id| registry.get(id).is_some()));
    }
    errors
}

pub fn list_runs(
    config: &Config,
    request: &WorkflowRunListRequest,
) -> Result<WorkflowRunListResponse> {
    Ok(list_workflow_runs(&config.workspace_dir, request)?)
}

pub fn get_run(config: &Config, id: &str) -> Result<Option<WorkflowRun>> {
    Ok(get_workflow_run(&config.workspace_dir, id)?)
}

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
