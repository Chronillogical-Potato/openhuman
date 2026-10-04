//! `context.md`: the compiled memory brief, one per memory node, and its
//! injection.
//!
//! [`refresh_for`] compiles a brief for one node from the bound engine with
//! `tinymemory-context` (four default briefs plus recent learnings, trimmed to
//! `[memory.context] budget_tokens`), reading only that node's reach — its own
//! memory and the nodes it inherits — and writes it beside a
//! `context_state.json` holding its timestamp and token count:
//!
//! - the root (the main agent, shared by everyone): `<workspace>/memory/context.md`;
//! - any other node: `<workspace>/memory/context/<kind>-<id>/…/context.md`,
//!   one directory per segment (`context/agent-researcher/context.md`).
//!
//! The `memory_context_refresh` cron job refreshes the root and every node
//! that has a document or a `[memory.agents.<id>]` entry ([`refresh_all`]);
//! `memory_context_refresh` refreshes one node on demand.
//!
//! [`injection_block`] is what the session host prepends to the first user
//! message of a **new** session, wrapped in `<memory-context>…</memory-context>`:
//! the acting agent's own document ([`super::scope::current`]), or, until
//! that is first compiled (which it then starts in the background), the
//! nearest ancestor's. A resumed session never gets it again: its transcript
//! is frozen.

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tinymemory::context::{Brief, ContextCompiler, ContextSpec, DEFAULT_LEARNINGS_LIMIT};
use tinymemory::{Namespace, Reach, Segment, SegmentKind};

use crate::config::Config;

use super::engine;
use super::error::{MemoryError, MemoryResult};
use super::types::{ContextSetParams, ContextView};

/// Fewest minutes between scheduled recompiles.
pub const MIN_INTERVAL_MINS: u32 = 5;

/// Smallest token budget.
pub const MIN_BUDGET_TOKENS: u32 = 100;

/// Largest token budget.
pub const MAX_BUDGET_TOKENS: u32 = 32_000;

/// Opening tag of the injected block.
pub const OPEN_TAG: &str = "<memory-context>";

/// Closing tag of the injected block.
pub const CLOSE_TAG: &str = "</memory-context>";

#[derive(Debug, Default, Serialize, Deserialize)]
struct ContextState {
    generated_at: Option<DateTime<Utc>>,
    tokens: usize,
}

fn memory_dir(workspace_dir: &Path) -> PathBuf {
    workspace_dir.join("memory")
}

/// The directory holding every non-root node's document.
fn nodes_dir(workspace_dir: &Path) -> PathBuf {
    memory_dir(workspace_dir).join("context")
}

/// The directory `namespace`'s document lives in.
fn node_dir(workspace_dir: &Path, namespace: &Namespace) -> PathBuf {
    if namespace.is_root() {
        return memory_dir(workspace_dir);
    }
    let mut dir = nodes_dir(workspace_dir);
    for segment in namespace.segments() {
        dir.push(format!("{}-{}", segment.kind().as_str(), segment.id()));
    }
    dir
}

/// Where the root's `context.md` lives.
#[must_use]
pub fn context_path(workspace_dir: &Path) -> PathBuf {
    context_path_for(workspace_dir, &Namespace::ROOT)
}

/// Where `namespace`'s `context.md` lives.
#[must_use]
pub fn context_path_for(workspace_dir: &Path, namespace: &Namespace) -> PathBuf {
    node_dir(workspace_dir, namespace).join("context.md")
}

fn state_path(workspace_dir: &Path, namespace: &Namespace) -> PathBuf {
    node_dir(workspace_dir, namespace).join("context_state.json")
}

fn read_state(workspace_dir: &Path, namespace: &Namespace) -> ContextState {
    std::fs::read_to_string(state_path(workspace_dir, namespace))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn read_markdown(workspace_dir: &Path, namespace: &Namespace) -> String {
    std::fs::read_to_string(context_path_for(workspace_dir, namespace)).unwrap_or_default()
}

/// `memory_context_get` for the root.
#[must_use]
pub fn view(config: &Config) -> ContextView {
    view_for(config, &Namespace::ROOT)
}

/// `memory_context_get` for `namespace`.
#[must_use]
pub fn view_for(config: &Config, namespace: &Namespace) -> ContextView {
    let state = read_state(&config.workspace_dir, namespace);
    let settings = &config.memory.context;
    ContextView {
        namespace: namespace.to_string(),
        markdown: read_markdown(&config.workspace_dir, namespace),
        tokens: state.tokens,
        generated_at: state.generated_at,
        interval_mins: settings.interval_mins,
        budget_tokens: settings.budget_tokens,
        enabled: settings.enabled,
    }
}

/// Every node with a compiled document, root first.
#[must_use]
pub fn compiled_nodes(workspace_dir: &Path) -> Vec<Namespace> {
    let mut found = vec![Namespace::ROOT];
    collect_nodes(&nodes_dir(workspace_dir), &Namespace::ROOT, &mut found);
    found
}

fn collect_nodes(dir: &Path, parent: &Namespace, found: &mut Vec<Namespace>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut children: Vec<(Namespace, PathBuf)> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let (kind, id) = name.split_once('-')?;
            let kind = SegmentKind::ALL.into_iter().find(|k| k.as_str() == kind)?;
            let node = parent.child(Segment::new(kind, id).ok()?).ok()?;
            Some((node, entry.path()))
        })
        .collect();
    children.sort();
    for (node, path) in children {
        if path.join("context.md").is_file() {
            found.push(node.clone());
        }
        collect_nodes(&path, &node, found);
    }
}

/// The spec compiled for the root.
#[must_use]
pub fn spec_for(config: &Config) -> ContextSpec {
    spec_for_node(config, &Namespace::ROOT)
}

/// The spec compiled for `namespace`: the node's own memory and the nodes it
/// inherits.
#[must_use]
pub fn spec_for_node(config: &Config, namespace: &Namespace) -> ContextSpec {
    ContextSpec {
        budget_tokens: config.memory.context.budget_tokens.max(MIN_BUDGET_TOKENS) as usize,
        briefs: Brief::defaults(),
        learnings_limit: DEFAULT_LEARNINGS_LIMIT,
        reach: Some(Reach::of(namespace.clone())),
    }
}

/// Compiles the root's `context.md` from the bound engine and writes it.
pub async fn refresh(config: &Config) -> MemoryResult<ContextView> {
    refresh_for(config, &Namespace::ROOT).await
}

/// Compiles `namespace`'s `context.md` from the bound engine and writes it.
pub async fn refresh_for(config: &Config, namespace: &Namespace) -> MemoryResult<ContextView> {
    let bound = engine::resolve(config).engine()?;
    refresh_node_with(config, namespace, &*bound.engine, ContextCompiler::new()).await
}

/// Refreshes the root and every node with a document or a configured agent,
/// skipping agents whose context is off. Returns how many were compiled.
pub async fn refresh_all(config: &Config) -> usize {
    let mut nodes = compiled_nodes(&config.workspace_dir);
    for (agent_id, settings) in &config.memory.agents {
        let node = super::scope::namespace_for(config, agent_id, None);
        if settings.context != Some(false) && !nodes.contains(&node) {
            nodes.push(node);
        }
    }
    let mut compiled = 0;
    for node in nodes {
        match refresh_for(config, &node).await {
            Ok(_) => compiled += 1,
            Err(error) => {
                tracing::debug!(namespace = %node, code = error.code(), "[memory:context] refresh skipped");
            }
        }
    }
    compiled
}

/// [`refresh`] against an explicit engine and compiler.
pub async fn refresh_with(
    config: &Config,
    engine: &dyn tinymemory::MemoryEngine,
    compiler: ContextCompiler,
) -> MemoryResult<ContextView> {
    refresh_node_with(config, &Namespace::ROOT, engine, compiler).await
}

/// [`refresh_for`] against an explicit engine and compiler.
pub async fn refresh_node_with(
    config: &Config,
    namespace: &Namespace,
    engine: &dyn tinymemory::MemoryEngine,
    compiler: ContextCompiler,
) -> MemoryResult<ContextView> {
    let doc = compiler
        .compile(engine, &spec_for_node(config, namespace))
        .await
        .map_err(|error| MemoryError::invalid(error.to_string()))?;
    let dir = node_dir(&config.workspace_dir, namespace);
    let state = ContextState {
        generated_at: Some(doc.generated_at),
        tokens: doc.tokens,
    };
    let write = std::fs::create_dir_all(&dir)
        .and_then(|()| {
            std::fs::write(
                context_path_for(&config.workspace_dir, namespace),
                &doc.markdown,
            )
        })
        .and_then(|()| {
            let json = serde_json::to_vec_pretty(&state).map_err(std::io::Error::other)?;
            std::fs::write(state_path(&config.workspace_dir, namespace), json)
        });
    write.map_err(|error| MemoryError::Engine(format!("writing context.md failed: {error}")))?;
    tracing::info!(
        engine = %doc.engine,
        namespace = %namespace,
        tokens = doc.tokens,
        refs = doc.refs.len(),
        "[memory:context] context.md compiled"
    );
    Ok(view_for(config, namespace))
}

/// Applies `memory_context_set` to `config`; the caller persists it.
pub fn apply_set(config: &mut Config, params: &ContextSetParams) -> MemoryResult<()> {
    let settings = &mut config.memory.context;
    if let Some(interval) = params.interval_mins {
        if interval < MIN_INTERVAL_MINS {
            return Err(MemoryError::invalid(format!(
                "interval_mins must be at least {MIN_INTERVAL_MINS}"
            )));
        }
        settings.interval_mins = interval;
    }
    if let Some(budget) = params.budget_tokens {
        if !(MIN_BUDGET_TOKENS..=MAX_BUDGET_TOKENS).contains(&budget) {
            return Err(MemoryError::invalid(format!(
                "budget_tokens must be between {MIN_BUDGET_TOKENS} and {MAX_BUDGET_TOKENS}"
            )));
        }
        settings.budget_tokens = budget;
    }
    if let Some(enabled) = params.enabled {
        settings.enabled = enabled;
    }
    Ok(())
}

/// Strips a leading `---` frontmatter block.
fn strip_frontmatter(markdown: &str) -> &str {
    let Some(rest) = markdown.strip_prefix("---\n") else {
        return markdown;
    };
    match rest.find("\n---\n") {
        Some(end) => &rest[end + "\n---\n".len()..],
        None => markdown,
    }
}

/// Whether the acting agent gets a compiled context: `[memory.context]
/// enabled`, overridden by `[memory.agents.<id>] context`.
fn context_enabled(config: &Config, agent_id: Option<&str>) -> bool {
    agent_id
        .and_then(|id| config.memory.agents.get(id))
        .and_then(|settings| settings.context)
        .unwrap_or(config.memory.context.enabled)
}

/// The block a new session's first user message is prefixed with, or `None`
/// when context is disabled, memory is off, or there is nothing compiled.
///
/// The document is the acting agent's ([`super::scope::current`]; the root's
/// outside any agent). A node without one yet falls back to its nearest
/// ancestor's and has its own compiled in the background, for its next
/// session.
#[must_use]
pub fn injection_block(config: &Config) -> Option<String> {
    let identity = super::scope::current().unwrap_or_else(super::scope::MemoryIdentity::root);
    if !context_enabled(config, identity.agent_id.as_deref()) {
        return None;
    }
    if !engine::is_on(config) {
        tracing::debug!("[memory:context] memory off; no context injected");
        return None;
    }
    let own = &identity.namespace(config);
    let inherit = identity.inherit(config);
    if !own.is_root() && !context_path_for(&config.workspace_dir, own).is_file() {
        compile_in_background(config, own);
    }
    let mut node = Some(own.clone());
    let markdown = loop {
        let Some(current) = node else {
            break String::new();
        };
        let markdown = read_markdown(&config.workspace_dir, &current);
        if !markdown.trim().is_empty() || !inherit {
            break markdown;
        }
        node = current.parent();
    };
    let body = strip_frontmatter(&markdown).trim();
    if body.is_empty() {
        return None;
    }
    tracing::debug!(
        namespace = %own,
        chars = body.len(),
        "[memory:context] context.md injected"
    );
    Some(format!("{OPEN_TAG}\n{body}\n{CLOSE_TAG}"))
}

/// Starts compiling `namespace`'s first document, when a runtime is there.
fn compile_in_background(config: &Config, namespace: &Namespace) {
    let Ok(handle) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let config = config.clone();
    let namespace = namespace.clone();
    tracing::debug!(namespace = %namespace, "[memory:context] compiling a first context.md");
    handle.spawn(async move {
        if let Err(error) = refresh_for(&config, &namespace).await {
            tracing::debug!(namespace = %namespace, code = error.code(), "[memory:context] first compile skipped");
        }
    });
}

/// Prepends [`injection_block`] (when there is one) to a new session's first
/// user message.
#[must_use]
pub fn prepend_to_first_message(config: &Config, message: &str) -> String {
    match injection_block(config) {
        Some(block) => format!("{block}\n\n{message}"),
        None => message.to_string(),
    }
}

#[cfg(test)]
#[path = "context_tests.rs"]
mod tests;
