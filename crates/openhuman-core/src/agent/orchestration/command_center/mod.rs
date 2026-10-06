//! Background agent command center (issue #3373).
//!
//! A read-only product surface over the durable run ledger: it lists recent
//! background agent runs grouped by a normalized status model (needs-input /
//! working / completed / failed / stopped) and lets a reviewer stop, retry,
//! continue or follow up on a run. The projection, bucket mapping and control
//! transitions live upstream in
//! `tinyagents_session::run_ledger::command_center`; this module is the
//! `openhuman.agent_work_*` RPC surface and the agent-registry display-name
//! lookup.

mod schemas;

pub use schemas::{
    all_controller_schemas as all_command_center_controller_schemas,
    all_registered_controllers as all_command_center_registered_controllers,
};
