//! The agents that keep records in their own storage scope, and a way for
//! background work to visit each of them.
//!
//! Work done inside an agent's turn runs under that agent's `CoreContext`
//! (`session_agent`), so with a storage backend installed its cron jobs,
//! flows, approvals and the rest land in that agent's scope. Background work
//! — the cron scheduler, pollers, boot sweeps — runs under the process
//! default context, which names no agent, and on its own would only ever see
//! the `local` scope. [`for_each_scope`] closes that gap: it runs a step once
//! for `local` and once under each known agent's context.
//!
//! An agent is known when it is live in this process
//! (`core::runtime::AgentContextRegistry`, which embed agents register on
//! build) — its own context is used, with its configuration, policy and
//! tools — or when an earlier process recorded its id in the backend's
//! `local` scope (`storage_agents`, written by [`record`] when the agent
//! registers), so a restart still visits agents the host has not re-created
//! yet; those are visited under the default context acting for them
//! (`CoreContext::for_agent`).
//!
//! Without a backend [`for_each_agent`] visits nothing: the SQLite stores
//! these loops read do not split by agent. In SaaS mode the process has no
//! `local` scope and per-user background work is driven by
//! `user_agents::background`, so agent ids are not recorded and
//! [`for_each_scope`] skips `local`.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex};

use serde_json::json;
use tinystoragedrivers::{CollectionSpec, Precondition, Query, Scope};

use super::{block_on, installed, DocumentStoreExt, StorageBackend};
use crate::core::runtime::{AgentContextRegistry, CoreContext};

/// The `local`-scope collection recording which agents have their own scope.
const AGENTS: &str = "storage_agents";

/// `(backend, agent id)` pairs this process has already recorded, the
/// backend identified by its address (see [`backend_key`]).
static RECORDED: LazyLock<Mutex<HashSet<(usize, String)>>> = LazyLock::new(Default::default);

/// Identifies `backend` within this process. [`reset_recorded`] runs on every
/// install and clear, so an address reused by a later backend never inherits
/// an earlier one's records.
fn backend_key(backend: &Arc<dyn StorageBackend>) -> usize {
    Arc::as_ptr(backend).cast::<()>() as usize
}

/// Forgets which agents were recorded, so the next [`record`] writes them to
/// the backend now installed (or removed). Called by [`super::install`] and
/// [`super::clear`]: the record cache describes one backend, not the process.
pub(super) fn reset_recorded() {
    RECORDED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clear();
}

/// Records every live agent in the backend — for agents registered before
/// the host installed its backend. Called by [`super::install`].
pub(super) fn record_live() {
    for (agent, _) in AgentContextRegistry::live() {
        record(&agent);
    }
}

/// Writes `agent` to the backend's `storage_agents` collection, once per
/// process. `AgentContextRegistry::register` calls it for every agent. Best
/// effort: a failure is logged and retried on the next registration, and
/// only costs a restarted process its visits to that agent until the agent
/// registers again.
pub fn record(agent: &str) {
    if crate::core::runtime::mode::is_saas() {
        return;
    }
    let Some(backend) = installed() else {
        return;
    };
    record_in(backend, agent);
}

/// [`record`] against an explicit `backend`.
fn record_in(backend: Arc<dyn StorageBackend>, agent: &str) {
    let key = (backend_key(&backend), agent.to_string());
    if RECORDED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(&key)
    {
        return;
    }
    let id = agent.to_string();
    let result = block_on(async move {
        let docs = Arc::clone(backend.for_scope(&Scope::local())?.documents());
        docs.ensure_collection(&CollectionSpec::new(AGENTS)).await?;
        docs.put(AGENTS, &id, json!({}), Precondition::None).await?;
        Ok(())
    });
    match result {
        Ok(()) => {
            RECORDED
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(key);
            tracing::debug!(%agent, "[storage::agents] recorded agent scope");
        }
        Err(error) => {
            tracing::warn!(%agent, %error, "[storage::agents] could not record agent scope");
        }
    }
}

/// Agent ids recorded in the backend by this or an earlier process.
fn recorded(backend: Arc<dyn StorageBackend>) -> Vec<String> {
    let result = block_on(async move {
        let docs = Arc::clone(backend.for_scope(&Scope::local())?.documents());
        docs.ensure_collection(&CollectionSpec::new(AGENTS)).await?;
        let stored = docs.query_all(AGENTS, &Query::all()).await?;
        Ok(stored.into_iter().map(|doc| doc.id).collect::<Vec<_>>())
    });
    result.unwrap_or_else(|error| {
        tracing::warn!(%error, "[storage::agents] could not list recorded agent scopes");
        Vec::new()
    })
}

/// Every agent background work should visit, each with the context to visit
/// it under: the live ones (`AgentContextRegistry`), then — with a storage
/// backend installed, outside SaaS mode — every agent recorded by this or an
/// earlier process, under the current context acting for it.
pub fn contexts() -> Vec<(String, Arc<CoreContext>)> {
    let backend = if crate::core::runtime::mode::is_saas() {
        None
    } else {
        installed()
    };
    contexts_in(backend, CoreContext::current().as_ref())
}

/// [`contexts`] with the backend whose recorded agents are visited, and the
/// context that acts for an agent with no live one, made explicit (`None`
/// visits live contexts only).
fn contexts_in(
    backend: Option<Arc<dyn StorageBackend>>,
    fallback: Option<&Arc<CoreContext>>,
) -> Vec<(String, Arc<CoreContext>)> {
    let mut contexts: BTreeMap<String, Arc<CoreContext>> =
        AgentContextRegistry::live().into_iter().collect();
    if let (Some(backend), Some(fallback)) = (backend, fallback) {
        for agent in recorded(backend) {
            contexts
                .entry(agent.clone())
                .or_insert_with(|| fallback.for_agent(&agent));
        }
    }
    contexts.into_iter().collect()
}

/// The context to act for `agent` under: its live context when one exists,
/// else the current context acting for it (`CoreContext::for_agent`).
pub fn context_for(agent: &str) -> Option<Arc<CoreContext>> {
    AgentContextRegistry::get(agent)
        .or_else(|| CoreContext::current().map(|current| current.for_agent(agent)))
}

/// Runs `fut` acting for `agent` when there is one — background work that
/// learned whose record it is handling (a device's pairing agent, an event's
/// publisher) re-enters that agent's scope — and as-is otherwise.
pub async fn within_agent<F: Future>(agent: Option<&str>, fut: F) -> F::Output {
    match agent.and_then(context_for) {
        Some(context) => CoreContext::scope(context, fut).await,
        None => fut.await,
    }
}

/// The scope a record lives in, for background work that holds only its id
/// (an event naming a flow, a job, a device): the first scope — `local`
/// first, then each known agent ([`for_each_scope`]) — where `probe` finds
/// it. `Some(None)` is `local`, `Some(Some(agent))` an agent, `None` nowhere.
pub async fn find_owner<F, Fut>(label: &str, probe: F) -> Option<Option<String>>
where
    F: Fn() -> Fut,
    Fut: Future<Output = bool>,
{
    for_each_scope(label, probe)
        .await
        .into_iter()
        .find_map(|(agent, found)| found.then_some(agent))
}

/// Runs `step` for every storage scope background work must cover: once
/// under the current context (the `local` scope, outside SaaS mode), then
/// once per known agent ([`for_each_agent`]). Each result is returned with
/// the agent it ran for (`None` for `local`).
///
/// `label` names the caller in logs.
pub async fn for_each_scope<T, F, Fut>(label: &str, step: F) -> Vec<(Option<String>, T)>
where
    F: Fn() -> Fut,
    Fut: Future<Output = T>,
{
    let mut results = Vec::new();
    if !crate::core::runtime::mode::is_saas() {
        results.push((None, step().await));
    }
    for (agent, value) in for_each_agent(label, step).await {
        results.push((Some(agent), value));
    }
    results
}

/// Runs `step` once under each known agent's context, one after another —
/// only when a storage backend is installed, since without one the stores do
/// not split by agent and the `local` pass already covers everything.
///
/// For a loop that handles the `local` scope itself (the cron scheduler keeps
/// its process-wide health tracking there) and needs the agents on top.
pub async fn for_each_agent<T, F, Fut>(label: &str, step: F) -> Vec<(String, T)>
where
    F: Fn() -> Fut,
    Fut: Future<Output = T>,
{
    if installed().is_none() {
        return Vec::new();
    }
    let mut results = Vec::new();
    for (agent, context) in contexts() {
        tracing::trace!(%agent, label, "[storage::agents] visiting agent scope");
        let value = CoreContext::scope(context, step()).await;
        results.push((agent, value));
    }
    results
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
