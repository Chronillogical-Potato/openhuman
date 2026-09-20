//! Hosted-context-preserving dispatch for packed skill tools.

use super::dispatch::DelegationDispatch;
use async_trait::async_trait;
use std::sync::Arc;
use tinyagents_harness::context::RunContext;
use tinyagents_harness::tool::{ToolDispatch, ToolExecutionContext};
use tinytools::{ToolCallOptions, ToolResult};

/// Typed dispatch for `use_skill` when its selected inner tool is a synthesized
/// delegation. Plain `Tool::execute_with_context` cannot carry the hosted
/// parent run context that a sub-agent needs, so route those inner calls back
/// through [`DelegationDispatch`] and leave every ordinary packed tool on the
/// proxy's existing execution path.
pub(crate) struct UseSkillDispatch {
    tool: Arc<dyn tinytools::Tool>,
    tool_sets: Vec<Arc<Vec<Box<dyn tinytools::Tool>>>>,
}

impl UseSkillDispatch {
    pub(crate) fn new(
        tool: Arc<dyn tinytools::Tool>,
        tool_sets: Vec<Arc<Vec<Box<dyn tinytools::Tool>>>>,
    ) -> Self {
        Self { tool, tool_sets }
    }
}

#[async_trait]
impl ToolDispatch<(), crate::agent::tinyagents::host::OpenHumanRunContext> for UseSkillDispatch {
    fn tool(&self) -> Arc<dyn tinytools::Tool> {
        self.tool.clone()
    }

    async fn execute(
        &self,
        state: &(),
        arguments: serde_json::Value,
        options: ToolCallOptions,
        parent: &RunContext<crate::agent::tinyagents::host::OpenHumanRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let skill = arguments.get("skill").and_then(serde_json::Value::as_str);
        let inner_name = arguments.get("tool").and_then(serde_json::Value::as_str);
        if let (Some(skill), Some(inner_name)) = (skill, inner_name) {
            let belongs_to_pack = crate::tools::toolpacks::pack_for_tool(inner_name)
                .is_some_and(|pack| pack.id == skill);
            if belongs_to_pack {
                if let Some(inner) =
                    crate::agent::tinyagents::tools::CanonicalSharedToolAdapter::for_name(
                        self.tool_sets.clone(),
                        inner_name,
                    )
                    .map(Arc::new)
                {
                    if let Some(dispatch) = DelegationDispatch::for_tool(inner) {
                        let inner_args = arguments
                            .get("args")
                            .cloned()
                            .unwrap_or_else(|| serde_json::json!({}));
                        return dispatch.execute(state, inner_args, options, parent).await;
                    }
                }
            }
        }

        let context = ToolExecutionContext::from_run_context(parent, _call_id.clone());
        self.tool
            .execute_with_context(arguments, options, Some(&context))
            .await
    }
}
