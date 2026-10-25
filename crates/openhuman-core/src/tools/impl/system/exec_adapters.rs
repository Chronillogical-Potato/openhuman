//! OpenHuman policy adapters for TinyAgents' generic execution tools.

use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde_json::Value;
use tinyagents_harness_tools::exec::{
    ExecAuditEvent, ExecAuditOutcome, ExecAuthorizationRequest, ExecBackendCapability, ExecDenied,
    ExecGate, ExecOperation, ExecPermit, ExecRefusalKind, NativeExecCapability,
};
use tinytools::ToolRunContext;

use crate::{
    agent::tinyagents::host::run_context::approval_for_tool,
    security::{AuditLogger, CommandExecutionLog, SecurityPolicy},
};

/// Host-owned gate for package installation. The generic TinyAgents tool owns
/// argument validation and process-result formatting; OpenHuman owns whether
/// a host install is permitted.
pub(super) struct InstallGate {
    pub security: Arc<SecurityPolicy>,
    pub audit: Arc<AuditLogger>,
    pub environment: std::collections::BTreeMap<std::ffi::OsString, std::ffi::OsString>,
}

#[async_trait]
impl ExecGate for InstallGate {
    async fn prepare_authorization(&self, _args: &Value) -> anyhow::Result<Option<Value>> {
        Ok(None)
    }

    fn external_effect_with_args(&self, _args: &Value) -> bool {
        true
    }

    async fn authorize(
        &self,
        request: &ExecAuthorizationRequest,
        context: Option<&dyn ToolRunContext>,
    ) -> Result<ExecPermit, ExecDenied> {
        let ExecOperation::Install { .. } = &request.operation else {
            return Err(ExecDenied(
                "install gate received a non-install operation".into(),
            ));
        };
        if crate::core::runtime::is_saas() {
            return Err(ExecDenied(
                "[policy-denied] Package installation is unavailable in SaaS mode.".into(),
            ));
        }
        if !approval_for_tool(context).is_interactive() {
            return Err(ExecDenied(
                "[policy-denied] install_tool requires interactive approval and is not available in autonomous (background) turns.".into(),
            ));
        }
        if !self.security.allow_tool_install {
            return Err(ExecDenied(
                "[policy-denied] OS package installation is disabled. Enable it in the agent access settings (Full access mode / allow_tool_install) before installing tools.".into(),
            ));
        }
        let ExecOperation::Install { package, manager } = &request.operation else {
            unreachable!("operation checked above")
        };
        let command = format!("{manager} install {package}");
        match self.security.check_command(crate::security::policy::CommandQuery {
            command: &command,
            declared: Some(crate::security::policy::CommandClass::Install),
            flow: crate::security::policy::CommandFlow::Harness,
            stage: crate::security::policy::CommandStage::Execute,
        }).await {
            Ok(verdict) if verdict.gate != crate::security::policy::GateDecision::Block => {}
            Ok(_) => return Err(ExecDenied("[policy-blocked] Install action denied by TinySecurity.".into())),
            Err(reason) => return Err(ExecDenied(reason.to_string())),
        }
        Ok(ExecPermit {
            action_dir: PathBuf::from(&self.security.workspace_dir),
            environment: self.environment.clone(),
            backend_selection: "native".into(),
            backend_capability: ExecBackendCapability::new(NativeExecCapability),
        })
    }

    fn record(&self, event: ExecAuditEvent) {
        let (command, risk_level) = match &event.operation {
            ExecOperation::Install { package, manager } => {
                (format!("{manager} install {package}"), "install")
            }
            ExecOperation::Shell { command, category } => {
                (command.clone(), category.as_deref().unwrap_or("unknown"))
            }
        };
        let success = matches!(
            event.outcome,
            ExecAuditOutcome::Completed { success: true, .. }
        );
        if let Err(error) = self.audit.log_command_event(CommandExecutionLog {
            channel: "tool:install_tool",
            command: &command,
            risk_level,
            approved: event.approval_required,
            allowed: event.allowed,
            success,
            duration_ms: u64::try_from(event.duration.as_millis()).unwrap_or(u64::MAX),
        }) {
            tracing::warn!(error = %error, "[install_tool] failed to persist execution audit event");
        }
        if let ExecAuditOutcome::BackendRefused {
            kind: ExecRefusalKind::BackendUnavailable,
        } = event.outcome
        {
            tracing::warn!("[install_tool] selected execution backend is unavailable");
        }
    }
}
