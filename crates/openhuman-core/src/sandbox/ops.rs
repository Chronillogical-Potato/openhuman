//! Sandbox backend operations split between host policy and execution adapters.

mod exec;
mod policy;

pub use exec::execute_in_sandbox;
pub(crate) use exec::execute_in_sandbox_yielding;
#[cfg(test)]
pub(crate) use exec::execute_unsandboxed;
#[cfg(any(feature = "modules", test))]
pub(crate) use policy::local_status_for_jail;
pub(crate) use policy::{
    apply_requested_backend, create_sandbox_backend_with_config, sandbox_component_off,
};
pub use policy::{
    command_requires_sandbox, create_sandbox_backend, resolve_sandbox_policy,
    SANDBOX_ENV_PASSTHROUGH, SANDBOX_OFF_ENV,
};
#[cfg(test)]
pub(crate) use policy::{config_disables_sandbox, sandbox_disabled_by_host, sandbox_off_value};

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
