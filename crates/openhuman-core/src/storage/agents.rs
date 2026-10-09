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
//! An agent is known when a context is derived for it in this process
//! ([`registered`], called by `CoreContext::derive_with`) — the live context
//! is used, with the agent's own configuration — or when an earlier process
//! did and recorded its id in the backend's `local` scope
//! (`storage_agents`), so a restart still visits agents the host has not
//! re-created yet; those are visited under the default context with the
//! agent swapped in (`CoreContext::for_agent`).
//!
//! Without a backend nothing here changes behavior: the SQLite stores do not
//! split by agent, so [`for_each_scope`] runs the step once. In SaaS mode the
//! process has no `local` scope and per-user background work is driven by
//! `user_agents::background`, so agent ids are not recorded and
//! [`for_each_scope`] visits only live agent contexts.

use std::collections::{BTreeMap, HashSet};
use std::future::Future;
use std::sync::{Arc, LazyLock, Mutex, Weak};

use serde_json::json;
use tinystoragedrivers::{CollectionSpec, Precondition, Query, Scope};

use super::{block_on, installed, DocumentStoreExt, StorageBackend};
use crate::core::runtime::CoreContext;

/// The `local`-scope collection recording which agents have their own scope.
const AGENTS: &str = "storage_agents";

/// Live agent contexts, by agent id.
///
/// Several contexts can name one agent (each `derive_with` makes one), so each
/// agent keeps all of its live contexts, newest last.
static LIVE: LazyLock<Mutex<BTreeMap<String, Vec<Weak<CoreContext>>>>> =
    LazyLock::new(Default::default);

/// Agent ids this process has already recorded in the backend.
static RECORDED: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(Default::default);

/// Records `context` as its agent's live context (when it names one) and
/// returns it unchanged. `CoreContext::derive_with` passes every derived
/// context through here.
pub fn registered(context: Arc<CoreContext>) -> Arc<CoreContext> {
    if let Some(agent) = context.session_agent() {
        let mut live = LIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Agents come and go; forget the ones whose contexts are all gone so
        // the registry stays as small as the set of live agents.
        live.retain(|_, entries| {
            entries.retain(|entry| entry.strong_count() > 0);
            !entries.is_empty()
        });
        live.entry(agent.to_string())
            .or_default()
            .push(Arc::downgrade(&context));
        drop(live);
        record(agent);
    }
    context
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

/// Records every live agent in the backend — for agents derived before the
/// host installed its backend. Called by [`super::install`].
pub(super) fn record_live() {
    let agents: Vec<String> = LIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .keys()
        .cloned()
        .collect();
    for agent in agents {
        record(&agent);
    }
}

/// Writes `agent` to the backend's `storage_agents` collection, once per
/// process. Best effort: a failure is logged and retried on the next
/// registration, and only costs a restarted process its visits to that
/// agent until the agent is derived again.
fn record(agent: &str) {
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
    if RECORDED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .contains(agent)
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
                .insert(agent.to_string());
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

/// The agents [`for_each_scope`] visits, each with the context to visit it
/// under: its live context when one exists, else `fallback` acting for it.
fn agent_contexts(fallback: Option<&Arc<CoreContext>>) -> Vec<(String, Arc<CoreContext>)> {
    let backend = if crate::core::runtime::mode::is_saas() {
        None
    } else {
        installed()
    };
    agent_contexts_in(backend, fallback)
}

/// [`agent_contexts`] with the backend whose recorded agents are visited
/// made explicit (`None` visits live contexts only).
fn agent_contexts_in(
    backend: Option<Arc<dyn StorageBackend>>,
    fallback: Option<&Arc<CoreContext>>,
) -> Vec<(String, Arc<CoreContext>)> {
    let mut contexts: BTreeMap<String, Arc<CoreContext>> = BTreeMap::new();
    {
        let mut live = LIVE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        live.retain(|agent, entries| {
            entries.retain(|entry| entry.strong_count() > 0);
            // The newest context that is still alive acts for the agent.
            if let Some(context) = entries.iter().rev().find_map(Weak::upgrade) {
                contexts.insert(agent.clone(), context);
            }
            !entries.is_empty()
        });
    }
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
    let live = LIVE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(agent)
        .and_then(|entries| entries.iter().rev().find_map(Weak::upgrade));
    live.or_else(|| CoreContext::current().map(|current| current.for_agent(agent)))
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
    let fallback = CoreContext::current();
    let mut results = Vec::new();
    for (agent, context) in agent_contexts(fallback.as_ref()) {
        tracing::trace!(%agent, label, "[storage::agents] visiting agent scope");
        let value = CoreContext::scope(context, async { step().await }).await;
        results.push((agent, value));
    }
    results
}

#[cfg(test)]
#[path = "agents_tests.rs"]
mod tests;
