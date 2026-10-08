//! Confinement of the ambient memory surface: the `openhuman.memory_*` RPCs
//! and the MCP memory tools, which dispatch through them.
//!
//! The agent `memory` tool ([`super::tools`]) always reads and forgets within
//! the acting identity's layout: `Reach::subtree(<root>)`. The RPCs used to
//! take whatever filter the caller sent, and with no `reach` an engine reads
//! every namespace, so an RPC or MCP caller could list, read and forget
//! another root's memory (another team's, or another host-bound agent's).
//!
//! Every RPC now resolves the identity in scope ([`super::scope::resolve_current`]:
//! the agent of a running turn, else the config's own root identity) and:
//!
//! - fills an unset `reach` with that identity's subtree ([`allowed_reach`]);
//! - keeps a caller's reach only when it is [`Reach::within`] the allowed
//!   one, and refuses a wider one with `INVALID_REQUEST`;
//! - places a `learn` with no namespace at the identity's learnings node and
//!   refuses one aimed outside the subtree.

use tinymemory_api::{MemoryMeta, MetaFilter, Namespace, Reach};

use crate::config::Config;

use super::error::{MemoryError, MemoryResult};
use super::scope;

/// What the caller under `config` may read and forget: the subtree of the
/// identity in scope's layout root.
#[must_use]
pub fn allowed_reach(config: &Config) -> Reach {
    Reach::subtree(scope::resolve_current(config).root().clone())
}

/// `reach` confined to `allowed`: unset becomes `allowed`, a reach within it
/// is kept.
///
/// # Errors
///
/// [`MemoryError::InvalidRequest`] when `reach` reads outside `allowed`.
pub fn confine_reach(reach: Option<Reach>, allowed: &Reach) -> MemoryResult<Reach> {
    match reach {
        None => Ok(allowed.clone()),
        Some(reach) if reach.within(allowed) => Ok(reach),
        Some(reach) => {
            tracing::warn!(
                asked = %reach.at,
                allowed = %allowed.at,
                "[memory:confine] refused a reach outside the caller's root"
            );
            Err(MemoryError::invalid(format!(
                "the reach `{}` is outside this caller's memory (`{}`)",
                reach.at, allowed.at
            )))
        }
    }
}

/// `filter` with its reach confined to `allowed` ([`confine_reach`]).
///
/// # Errors
///
/// [`MemoryError::InvalidRequest`] when the filter's reach reads outside
/// `allowed`.
pub fn confine_filter(filter: Option<MetaFilter>, allowed: &Reach) -> MemoryResult<MetaFilter> {
    let mut filter = filter.unwrap_or_default();
    filter.reach = Some(confine_reach(filter.reach.take(), allowed)?);
    Ok(filter)
}

/// The metadata a `memory_learn` caller sent, with its namespace inside the
/// identity in scope's layout: an unset namespace (the root) lands at the
/// layout's learnings node when that is not the root itself.
///
/// # Errors
///
/// [`MemoryError::InvalidRequest`] when the namespace is outside the
/// layout's subtree.
pub fn confine_learn_meta(config: &Config, meta: Option<MemoryMeta>) -> MemoryResult<MemoryMeta> {
    let resolved = scope::resolve_current(config);
    let allowed = Reach::subtree(resolved.root().clone());
    let mut meta = meta.unwrap_or_default();
    if meta.namespace == Namespace::ROOT {
        meta.namespace = resolved.layout.learnings().clone();
    }
    if !allowed.admits(&meta.namespace) {
        tracing::warn!(
            asked = %meta.namespace,
            allowed = %allowed.at,
            "[memory:confine] refused a learning outside the caller's root"
        );
        return Err(MemoryError::invalid(format!(
            "the namespace `{}` is outside this caller's memory (`{}`)",
            meta.namespace, allowed.at
        )));
    }
    Ok(meta)
}

#[cfg(test)]
#[path = "confine_tests.rs"]
mod tests;
