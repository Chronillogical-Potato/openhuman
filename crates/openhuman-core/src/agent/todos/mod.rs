//! OpenHuman adapters around the TinyAgents todo store.
//!
//! Design notes:
//! - **Per-thread scoped.** The list belongs to the conversation thread the
//!   turn runs in; there is no cross-thread or app-wide board.
//! - **In-memory scratch.** When no thread context is available the
//!   process-global scratch store is used (tool invocations outside a chat
//!   thread, tests).
//! - **Markdown output.** Tool results include a rendered representation for
//!   the agent transcript.

pub mod ops;
pub mod types;
