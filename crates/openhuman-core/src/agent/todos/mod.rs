//! OpenHuman adapters around the TinyAgents todo store.
//!
//! Design notes:
//! - **Per-thread scoped.** The current agent thread id (or an explicit
//!   thread id selects which board to mutate.
//! - **In-memory scratch.** When no thread context is available the
//!   process-global scratch store is used (legacy fallback for tool
//!   invocations outside a chat thread).
//! - **Markdown output.** Tool results include a rendered representation for
//!   the agent transcript.

pub mod ops;
pub mod tools;
pub mod types;
