//! Process-global core seams as builder options.
//!
//! The core exposes a handful of installers a host calls once per process:
//! the controller registry extension, the `tool_search` ranker, embedder
//! post-turn and tool hooks, the `run`/`serve` server launcher and the live
//! security policy. [`RuntimeBuilder`] takes each as an option, installs it on
//! `build()`, and restores what it can when the runtime is dropped:
//!
//! | Seam | Installed | On drop |
//! |---|---|---|
//! | [`controller_extension`](RuntimeBuilder::controller_extension) | before boot | kept: the registry has no removal; re-registering the same controllers is a no-op |
//! | [`tool_ranker`](RuntimeBuilder::tool_ranker) | before boot | the previous ranker is restored, if ours still holds the slot |
//! | [`post_turn_hook`](RuntimeBuilder::post_turn_hook) / [`tool_hook`](RuntimeBuilder::tool_hook) | before boot, replacing a same-named hook | newest first, while ours still holds the name: ours removed, a replaced same-named hook restored |
//! | [`server_launcher`](RuntimeBuilder::server_launcher) | before boot | kept: first install wins for the process |
//! | [`live_policy`](RuntimeBuilder::live_policy) | after boot | kept: the next boot (or a config reload) installs its own |
//!
//! The live policy goes in *after* boot because the core's bootstrap installs
//! one derived from the config; installing earlier would be overwritten. A
//! later `config.update_autonomy_settings` reload or channel startup rebuilds
//! the policy from config again — the override lasts until then.

use std::sync::Arc;

use openhuman_core::agent::hooks::{
    embedder_post_turn_hooks, embedder_tool_hooks, replace_embedder_post_turn_hook,
    replace_embedder_tool_hook, PostTurnHook, ToolHook,
};
use openhuman_core::agent::tinyagents::discovery::{
    clear_tool_ranker, install_tool_ranker, installed_tool_ranker,
};
use openhuman_core::core::all::{register_controller_extension, ControllerExtension};
use openhuman_core::core::server_launcher::{install_server_launcher, ServerLauncher};
use openhuman_core::security::SecurityPolicy;
use tinytools::ToolRanker;

use super::RuntimeBuilder;

/// The seam options a builder collected, not yet installed.
#[derive(Default)]
pub(crate) struct HostSeams {
    pub(crate) controller_extensions: Vec<ControllerExtension>,
    pub(crate) tool_ranker: Option<Arc<dyn ToolRanker>>,
    pub(crate) post_turn_hooks: Vec<Arc<dyn PostTurnHook>>,
    pub(crate) tool_hooks: Vec<Arc<dyn ToolHook>>,
    pub(crate) server_launcher: Option<ServerLauncher>,
    pub(crate) live_policy: Option<Arc<SecurityPolicy>>,
}

impl HostSeams {
    /// Install every pre-boot seam and return the guard that undoes the
    /// restorable ones.
    ///
    /// Controller extensions go first because they are the only fallible
    /// step; nothing restorable has been touched when one is refused. An
    /// extension accepted before a later one is refused stays registered —
    /// the registry has no removal.
    pub(crate) fn install(self) -> Result<InstalledSeams, String> {
        for ext in self.controller_extensions {
            let group = ext.group;
            register_controller_extension(ext).map_err(|error| {
                log::warn!("[embed][seams] controller extension refused group={group:?}: {error}");
                error
            })?;
            log::debug!("[embed][seams] controller extension registered group={group:?}");
        }

        if let Some(launcher) = self.server_launcher {
            install_server_launcher(launcher);
            log::debug!("[embed][seams] server launcher installed (process lifetime)");
        }

        let ranker = self.tool_ranker.map(|ranker| {
            let previous = installed_tool_ranker();
            log::debug!(
                "[embed][seams] tool ranker installed kind={} replaced={}",
                ranker.kind(),
                previous.is_some()
            );
            install_tool_ranker(Arc::clone(&ranker));
            (ranker, previous)
        });

        let post_turn_hooks = self
            .post_turn_hooks
            .into_iter()
            .map(|hook| {
                let name = hook.name().to_string();
                let previous = embedder_post_turn_hooks()
                    .into_iter()
                    .find(|existing| existing.name() == name);
                replace_embedder_post_turn_hook(&name, Some(Arc::clone(&hook)));
                log::debug!(
                    "[embed][seams] post-turn hook installed name={name} replaced={}",
                    previous.is_some()
                );
                InstalledHook {
                    name,
                    ours: hook,
                    previous,
                }
            })
            .collect();

        let tool_hooks = self
            .tool_hooks
            .into_iter()
            .map(|hook| {
                let name = hook.name().to_string();
                let previous = embedder_tool_hooks()
                    .into_iter()
                    .find(|existing| existing.name() == name);
                replace_embedder_tool_hook(&name, Some(Arc::clone(&hook)));
                log::debug!(
                    "[embed][seams] tool hook installed name={name} replaced={}",
                    previous.is_some()
                );
                InstalledHook {
                    name,
                    ours: hook,
                    previous,
                }
            })
            .collect();

        Ok(InstalledSeams {
            ranker,
            post_turn_hooks,
            tool_hooks,
            live_policy: self.live_policy,
            restore: true,
        })
    }
}

/// A ranker we installed, and what held the slot before it.
type InstalledRanker = (Arc<dyn ToolRanker>, Option<Arc<dyn ToolRanker>>);

/// A hook we installed under `name`, and what held that name before it.
struct InstalledHook<H: ?Sized> {
    name: String,
    ours: Arc<H>,
    previous: Option<Arc<H>>,
}

/// Seams a runtime installed. Dropping it restores the restorable ones; see
/// the module docs.
pub(crate) struct InstalledSeams {
    ranker: Option<InstalledRanker>,
    /// Our hooks, in install order, each with the same-named hook it replaced.
    post_turn_hooks: Vec<InstalledHook<dyn PostTurnHook>>,
    tool_hooks: Vec<InstalledHook<dyn ToolHook>>,
    /// Installed only once the core has booted.
    live_policy: Option<Arc<SecurityPolicy>>,
    restore: bool,
}

impl InstalledSeams {
    /// Install the live policy, if one was given, against the booted config's
    /// directories. Call after the core's bootstrap has installed its own.
    pub(crate) fn install_live_policy(
        &mut self,
        workspace_dir: &std::path::Path,
        action_dir: &std::path::Path,
    ) {
        if let Some(policy) = self.live_policy.take() {
            openhuman_core::security::live_policy::install(
                policy,
                workspace_dir.to_path_buf(),
                action_dir.to_path_buf(),
            );
            log::debug!("[embed][seams] live policy installed (until the next reload)");
        }
    }

    /// Whether a live policy is still waiting for the core to boot.
    pub(crate) fn has_pending_live_policy(&self) -> bool {
        self.live_policy.is_some()
    }

    /// Keep everything installed for the rest of the process — for an entry
    /// point that never returns control to a runtime ([`RuntimeBuilder::run_from_args`]).
    pub(crate) fn persist(mut self) {
        self.restore = false;
        log::debug!("[embed][seams] seams kept for the process lifetime");
    }
}

impl Drop for InstalledSeams {
    fn drop(&mut self) {
        if !self.restore {
            return;
        }
        if let Some((ours, previous)) = self.ranker.take() {
            let still_ours = installed_tool_ranker().is_some_and(|now| Arc::ptr_eq(&now, &ours));
            if still_ours {
                match previous {
                    Some(previous) => install_tool_ranker(previous),
                    None => clear_tool_ranker(),
                }
                log::debug!("[embed][seams] tool ranker restored");
            } else {
                log::debug!("[embed][seams] tool ranker replaced since build; left alone");
            }
        }
        // Newest first: a second same-named hook recorded the first as its
        // `previous`, so unwinding in install order would put the first back
        // over the original. And only while ours still holds the name, so a
        // hook installed after build is not clobbered.
        while let Some(hook) = self.post_turn_hooks.pop() {
            let current = embedder_post_turn_hooks()
                .into_iter()
                .find(|existing| existing.name() == hook.name);
            if current.is_some_and(|now| Arc::ptr_eq(&now, &hook.ours)) {
                replace_embedder_post_turn_hook(&hook.name, hook.previous);
                log::debug!("[embed][seams] post-turn hook restored name={}", hook.name);
            } else {
                log::debug!(
                    "[embed][seams] post-turn hook replaced since build; left alone name={}",
                    hook.name
                );
            }
        }
        while let Some(hook) = self.tool_hooks.pop() {
            let current = embedder_tool_hooks()
                .into_iter()
                .find(|existing| existing.name() == hook.name);
            if current.is_some_and(|now| Arc::ptr_eq(&now, &hook.ours)) {
                replace_embedder_tool_hook(&hook.name, hook.previous);
                log::debug!("[embed][seams] tool hook restored name={}", hook.name);
            } else {
                log::debug!(
                    "[embed][seams] tool hook replaced since build; left alone name={}",
                    hook.name
                );
            }
        }
    }
}

impl RuntimeBuilder {
    /// Contribute controllers to the core's registry, as the TinyHumans layer
    /// does for its hosted RPC proxies. May be called more than once.
    ///
    /// Registered before boot; a collision with a built-in or earlier
    /// extension fails the build with [`RuntimeError::Invalid`](super::RuntimeError::Invalid).
    /// The registry has no removal, so an extension outlives the runtime;
    /// registering identical controllers again is a no-op.
    pub fn controller_extension(mut self, extension: ControllerExtension) -> Self {
        self.seams.controller_extensions.push(extension);
        self
    }

    /// The process-wide `tool_search` ranker. Restored to the previous one
    /// when the runtime drops.
    pub fn tool_ranker(mut self, ranker: Arc<dyn ToolRanker>) -> Self {
        self.seams.tool_ranker = Some(ranker);
        self
    }

    /// A hook every agent session runs after each turn. Replaces a
    /// registered hook of the same [`name`](PostTurnHook::name); removed (and
    /// the replaced one restored) when the runtime drops.
    pub fn post_turn_hook(mut self, hook: Arc<dyn PostTurnHook>) -> Self {
        self.seams.post_turn_hooks.push(hook);
        self
    }

    /// A hook every agent session runs around each tool call. Same
    /// replacement and restore rules as [`post_turn_hook`](Self::post_turn_hook).
    pub fn tool_hook(mut self, hook: Arc<dyn ToolHook>) -> Self {
        self.seams.tool_hooks.push(hook);
        self
    }

    /// The launcher the CLI's `run` / `serve` subcommands start a server
    /// with. The first launcher installed in a process wins and stays.
    pub fn server_launcher(mut self, launcher: ServerLauncher) -> Self {
        self.seams.server_launcher = Some(launcher);
        self
    }

    /// Replace the live security policy the core installs at boot. Applied
    /// after boot; a later autonomy-settings reload rebuilds it from config.
    pub fn live_policy(mut self, policy: Arc<SecurityPolicy>) -> Self {
        self.seams.live_policy = Some(policy);
        self
    }
}

#[cfg(test)]
#[path = "seams_tests.rs"]
mod tests;
