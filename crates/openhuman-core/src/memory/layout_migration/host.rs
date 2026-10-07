//! What the migration needs from the host, behind one trait so the job is
//! tested without a TinyHumans session or a CortexDB server.
//!
//! The real host (the engine binding and the layout setting) implements it
//! over `memory::engine` and `memory::scope`; tests implement it over two
//! in-memory engines.

use async_trait::async_trait;

use super::copy::Engines;
use super::map::Placement;
use crate::config::Config;
use crate::memory::error::MemoryResult;

/// The host side of a layout migration.
#[async_trait]
pub trait LayoutHost: Send + Sync {
    /// The legacy-layout engine and the per-user engine for `config`'s
    /// account, on one endpoint and credential.
    ///
    /// # Errors
    ///
    /// When memory is off or the account has no per-user root.
    fn engines(&self, config: &Config) -> MemoryResult<Engines>;

    /// Where items go in the per-user tree.
    ///
    /// # Errors
    ///
    /// When the layout cannot be built.
    fn placement(&self, config: &Config) -> MemoryResult<Placement>;

    /// Whether reads and writes already use the per-user tree.
    fn is_switched(&self, config: &Config) -> bool;

    /// Moves reads and writes to the per-user tree.
    ///
    /// # Errors
    ///
    /// When the setting cannot be saved.
    fn switch(&self, config: &Config) -> MemoryResult<()>;

    /// Whether moving memory costs the user nothing right now (always, off
    /// the hosted engine).
    async fn free_now(&self, config: &Config) -> bool;

    /// Whether the legacy tree may be shared with other accounts on this
    /// machine (a self-hosted key), so taking it needs the user's consent.
    fn shared_legacy(&self, config: &Config) -> bool;
}
