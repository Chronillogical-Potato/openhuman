//! Confining a SaaS user's memory to the user's own tree.
//!
//! Every SaaS user agent runs with `[memory] root = user:<agent>`
//! (`user_agents::layout::agent_config`). The lifecycle hooks already read and
//! write below that root, but the memory operations — the `memory.*` RPCs and
//! the agent's memory tools — pass the caller's filter or namespace straight
//! to the engine. Users normally have engines of their own (each is bound to
//! the user's backend credential), so that alone keeps them apart; but users
//! may share an engine (a shared backend key, one CortexDB key), and then a
//! filter that reaches the tree's root would read everyone.
//!
//! So in SaaS mode, for a config that names a root:
//!
//! - every read reaches only the subtree below the root: a reach that already
//!   lies inside it keeps its node but loses ancestors (which would climb out
//!   of the root); any other reach becomes the root's subtree;
//! - every write lands inside the root: an item addressed elsewhere is moved
//!   to the root itself.
//!
//! Single-user processes and configs without a root are left alone.

use tinymemory_api::{MetaFilter, Namespace, Reach, StoreItem};

use crate::config::Config;

/// The root `config`'s memory is confined to, in SaaS mode.
pub fn confinement(config: &Config) -> Option<Namespace> {
    confinement_in(crate::core::runtime::is_saas(), config)
}

/// [`confinement`] as a pure function of the mode.
pub fn confinement_in(saas: bool, config: &Config) -> Option<Namespace> {
    if !saas {
        return None;
    }
    let raw = config.memory.root.as_deref()?.trim();
    match raw.parse::<Namespace>() {
        Ok(root) => Some(root),
        Err(error) => {
            // A user agent's root is forced and valid; failing closed here
            // would turn memory off, failing open would read everyone. Log it
            // and confine to a root nothing else uses.
            tracing::error!(root = raw, %error, "[memory:user_scope] invalid confinement root");
            Some(Namespace::ROOT)
        }
    }
}

/// Whether `namespace` is `root` or lies below it.
pub fn within(namespace: &Namespace, root: &Namespace) -> bool {
    namespace.segments().starts_with(root.segments())
}

/// `reach`, confined to `root`.
pub fn clamp_reach(reach: Option<Reach>, root: &Namespace) -> Reach {
    match reach {
        Some(reach) if within(&reach.at, root) => Reach {
            inherit: false,
            ..reach
        },
        _ => Reach::subtree(root.clone()),
    }
}

/// `filter`, with its reach confined to `root`.
pub fn clamp_filter(mut filter: MetaFilter, root: &Namespace) -> MetaFilter {
    filter.reach = Some(clamp_reach(filter.reach.take(), root));
    filter
}

/// `item`, addressed inside `root`.
pub fn clamp_item(item: &mut StoreItem, root: &Namespace) {
    if !within(&item.meta().namespace, root) {
        item.meta_mut().namespace = root.clone();
    }
}

#[cfg(test)]
#[path = "user_scope_tests.rs"]
mod tests;
