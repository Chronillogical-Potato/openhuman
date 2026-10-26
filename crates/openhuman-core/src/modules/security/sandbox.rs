//! Typed host calls for TinySecurity sandbox plans.

use tinysecurity_bus::{
    names, ConsumeSandboxPlanRequest2, ConsumeSandboxPlanResponse2, PolicyInfo,
    RegisterSandboxScopeRequest2, RetireSandboxScopeRequest2, SandboxPlanRequest2,
    SandboxPlanResponse2, SandboxScopeReceipt2,
};

use crate::config::Config;

/// Load TinySecurity and return the admitted sandbox object.
pub(crate) async fn object(config: &Config) -> Result<tinybus::module::ModuleObjectClient, String> {
    super::approval_object(config, std::time::Duration::from_secs(25)).await
}

/// Return the current policy generation, which plans and audit rows must bind.
pub(crate) async fn generation(
    object: &tinybus::module::ModuleObjectClient,
) -> Result<u64, String> {
    let info: PolicyInfo = object
        .call_confidential(names::methods::POLICY_INFO, ())
        .await
        .map_err(|_| "TinySecurity PolicyInfo failed")?;
    if info.generation == 0 {
        return Err("TinySecurity reported no policy generation".into());
    }
    Ok(info.generation)
}

/// Register one immutable subject sandbox scope.
pub(crate) async fn register_scope(
    object: &tinybus::module::ModuleObjectClient,
    request: RegisterSandboxScopeRequest2,
) -> Result<SandboxScopeReceipt2, String> {
    let expected = request.scope.scope_id.clone();
    let receipt: SandboxScopeReceipt2 = object
        .call_confidential(names::methods::REGISTER_SANDBOX_SCOPE2, (request,))
        .await
        .map_err(|_| "TinySecurity RegisterSandboxScope2 failed")?;
    if receipt.scope_id != expected {
        return Err("TinySecurity returned a receipt for another sandbox scope".into());
    }
    Ok(receipt)
}

/// Retire a runtime scope and all plans issued under it.
pub(crate) async fn retire_scope(
    object: &tinybus::module::ModuleObjectClient,
    scope_id: &str,
) -> Result<(), String> {
    let receipt: SandboxScopeReceipt2 = object
        .call_confidential(
            names::methods::RETIRE_SANDBOX_SCOPE2,
            (RetireSandboxScopeRequest2 {
                scope_id: scope_id.to_owned(),
            },),
        )
        .await
        .map_err(|_| "TinySecurity RetireSandboxScope2 failed")?;
    if receipt.scope_id != scope_id {
        return Err("TinySecurity returned a receipt for another sandbox scope".into());
    }
    Ok(())
}

/// Ask TinySecurity to plan one concrete sandbox operation.
pub(crate) async fn plan(
    object: &tinybus::module::ModuleObjectClient,
    request: SandboxPlanRequest2,
) -> Result<SandboxPlanResponse2, String> {
    object
        .call_confidential(names::methods::PLAN2, (request,))
        .await
        .map_err(|_| "TinySecurity Plan2 failed".to_owned())
}

/// Atomically reserve a plan immediately before execution.
pub(crate) async fn consume(
    object: &tinybus::module::ModuleObjectClient,
    request: ConsumeSandboxPlanRequest2,
) -> Result<ConsumeSandboxPlanResponse2, String> {
    object
        .call_confidential(names::methods::CONSUME_SANDBOX_PLAN2, (request,))
        .await
        .map_err(|_| "TinySecurity ConsumeSandboxPlan2 failed".to_owned())
}

