//! Contract 1.22 policy-gap calls used by host security policy.
//!
//! TinySecurity owns the answer and the action ledger. The host supplies only
//! immutable configuration derived from its policy object and fails closed when
//! the module cannot answer.

use std::path::PathBuf;
use std::sync::OnceLock;
#[cfg(test)]
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tinysecurity_bus::{BudgetDecision2, CommandPolicy, CommandPolicyId};

/// Whether to reserve an action or inspect the budget without reserving it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BudgetOp {
    /// Reserve one allowed action.
    Spend,
    /// Read the budget without reserving.
    Peek,
}

/// Whether a command answer is informational or reserves an allowed action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CommandOp {
    /// Classify without applying admission or spending budget.
    Classify,
    /// Apply the configured command gate and spend on allow.
    Check,
}

/// Port for the action budget, so host policy tests exercise the same boundary
/// as production without booting a module runtime.
#[async_trait]
pub(crate) trait BudgetPort: std::fmt::Debug + Send + Sync {
    /// Ask TinySecurity to spend or inspect the policy's hourly budget.
    async fn budget(
        &self,
        workspace_dir: PathBuf,
        policy: CommandPolicy,
        op: BudgetOp,
    ) -> Result<BudgetDecision2, String>;
}

#[derive(Debug)]
struct ModuleBudgetPort;

thread_local! {
    #[cfg(test)]
    static TEST_BUDGET_PORT: std::cell::RefCell<Option<Arc<dyn BudgetPort>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) struct BudgetPortGuard(Option<Arc<dyn BudgetPort>>);

#[cfg(test)]
impl Drop for BudgetPortGuard {
    fn drop(&mut self) {
        TEST_BUDGET_PORT.with(|slot| *slot.borrow_mut() = self.0.take());
    }
}

#[cfg(test)]
pub(crate) fn install_budget_port(port: Arc<dyn BudgetPort>) -> BudgetPortGuard {
    BudgetPortGuard(TEST_BUDGET_PORT.with(|slot| slot.borrow_mut().replace(port)))
}

/// Test implementation that returns a configured result and records each ask.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ScriptedBudgetPort {
    result: BudgetDecision2,
    asked: Mutex<Vec<(BudgetOp, u32)>>,
}

#[cfg(test)]
impl ScriptedBudgetPort {
    pub(crate) fn replying(result: BudgetDecision2) -> Arc<Self> {
        Arc::new(Self {
            result,
            asked: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn asked(&self) -> Vec<(BudgetOp, u32)> {
        self.asked.lock().expect("budget ask lock").clone()
    }
}

#[cfg(test)]
#[async_trait]
impl BudgetPort for ScriptedBudgetPort {
    async fn budget(
        &self,
        _workspace_dir: PathBuf,
        policy: CommandPolicy,
        op: BudgetOp,
    ) -> Result<BudgetDecision2, String> {
        self.asked
            .lock()
            .map_err(|_| "scripted budget port poisoned".to_owned())?
            .push((op, policy.max_actions_per_hour));
        Ok(self.result.clone())
    }
}

/// Ask the module to spend or peek. Test callers may install a scripted port;
/// production always uses the attested module client.
pub(crate) async fn budget(
    workspace_dir: PathBuf,
    policy: CommandPolicy,
    op: BudgetOp,
) -> Result<BudgetDecision2, String> {
    #[cfg(test)]
    if let Some(port) = TEST_BUDGET_PORT.with(|slot| slot.borrow().clone()) {
        return port.budget(workspace_dir, policy, op).await;
    }
    ModuleBudgetPort.budget(workspace_dir, policy, op).await
}

/// Classify or check a command under an immutable registered command scope.
pub(crate) async fn command_decision(
    workspace_dir: PathBuf,
    policy: CommandPolicy,
    command: String,
    declared_class: Option<tinysecurity_bus::CommandClass>,
    op: CommandOp,
) -> Result<tinysecurity_bus::CommandDecision, String> {
    let config = crate::core::runtime::DispatchCtx::ambient()
        .load_config()
        .await?;
    let client = super::client(&config)
        .await
        .map_err(|error| error.to_string())?;
    let scopes = COMMAND_SCOPES.get_or_init(|| tokio::sync::Mutex::new(Vec::new()));
    let mut scopes = scopes.lock().await;
    let policy_id = match scopes
        .iter()
        .find(|(workspace, registered, _)| workspace == &workspace_dir && registered == &policy)
        .map(|(_, _, id)| id.clone())
    {
        Some(id) => id,
        None => {
            let id = client
                .register_command_policy(policy.clone())
                .await
                .map_err(|error| error.to_string())?;
            scopes.push((workspace_dir, policy, id.clone()));
            id
        }
    };
    let request = tinysecurity_bus::CommandRequest {
        policy_id,
        command,
        declared_class,
    };
    match op {
        CommandOp::Classify => client.classify_command(request).await,
        CommandOp::Check => client.check_command(request).await,
    }
    .map_err(|error| error.to_string())
}

static COMMAND_SCOPES: OnceLock<
    tokio::sync::Mutex<Vec<(PathBuf, CommandPolicy, CommandPolicyId)>>,
> = OnceLock::new();

#[async_trait]
impl BudgetPort for ModuleBudgetPort {
    async fn budget(
        &self,
        workspace_dir: PathBuf,
        policy: CommandPolicy,
        op: BudgetOp,
    ) -> Result<BudgetDecision2, String> {
        let config = crate::core::runtime::DispatchCtx::ambient()
            .load_config()
            .await?;
        let client = super::client(&config)
            .await
            .map_err(|error| error.to_string())?;
        let scopes = COMMAND_SCOPES.get_or_init(|| tokio::sync::Mutex::new(Vec::new()));
        let mut scopes = scopes.lock().await;
        let policy_id = match scopes
            .iter()
            .find(|(workspace, registered, _)| workspace == &workspace_dir && registered == &policy)
            .map(|(_, _, id)| id.clone())
        {
            Some(id) => id,
            None => {
                let id = client
                    .register_command_policy(policy.clone())
                    .await
                    .map_err(|error| error.to_string())?;
                scopes.push((workspace_dir, policy, id.clone()));
                id
            }
        };
        let request = tinysecurity_bus::BudgetRequest2 {
            command_policy: policy_id,
        };
        match op {
            BudgetOp::Spend => client.spend_action2(request).await,
            BudgetOp::Peek => client.peek_budget2(request).await,
        }
        .map_err(|error| error.to_string())
    }
}
