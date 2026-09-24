//! `commands` — read-only command-palette listing for the chat composer's
//! slash-command menu (assistant-ui `composer-trigger-popover`, C5 of the
//! `assistant-ui-elements` plan).
//!
//! `commands.list` merges the fixed built-in slash commands with the live
//! skill and workflow catalogs (`skills.list` / `flows.list`), so the
//! frontend has one call to populate the menu instead of three. It is
//! deliberately read-only: naming a command here does not run it — a
//! built-in still dispatches through whatever RPC the frontend already uses
//! for it (`/plan` → `agent.set_run_mode`, etc.), and a skill/workflow
//! dispatches through `skills.run` / `flows.run` as it always did.

pub mod ops;
pub mod schemas;
pub mod types;

pub use schemas::{all_controller_schemas as all_commands_controller_schemas, schemas};
pub use types::{CommandEntry, CommandKind};

use crate::core::all::RegisteredController;

/// Registers `commands.list` with the controller registry (`core/all.rs`).
pub fn all_commands_registered_controllers() -> Vec<RegisteredController> {
    schemas::all_registered_controllers()
}
