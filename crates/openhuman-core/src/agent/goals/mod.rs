//! Thin OpenHuman host adapters for [`tinyagents_graph::goals`].
//!
//! Tinyagents owns the goal types, lifecycle, persistence, prompt rendering,
//! graph continuation, and native harness tools. This module contains the
//! remaining OpenHuman-specific adapters: workspace-store resolution, domain
//! events, heartbeat dispatch, and bridges for
//! OpenHuman's `Tool` and `StopHook` traits.

pub mod continuation;
pub mod migration;
pub mod runtime;
pub mod store;
pub mod tools;

pub use tinyagents_graph::goals::{ThreadGoal, ThreadGoalStatus};
pub use tools::{GoalCompleteTool, GoalGetTool, GoalSetTool};
