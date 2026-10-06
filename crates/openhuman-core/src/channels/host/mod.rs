//! OpenHuman implementation of the `tinychannels::host` capability boundary.
//!
//! [`build_channel_host`] assembles the concrete [`tinychannels::ChannelHost`]
//! from OpenHuman internals (voice, inference, approvals, conversation store,
//! shutdown registry, web event bus). [`build_provider_context`] wraps it into
//! the [`tinychannels::host::ProviderContext`] handed to channel providers.
//!
//! [`ChannelApprovalSurfaceSubscriber`] and [`ChannelTurnStateSubscriber`]
//! bridge OpenHuman events into the provider-independent approval surface and
//! remote-control turn state; `remote_control` backs `/status`, `/sessions`
//! and `/new`.
//!
//! Ported providers reach host capabilities through this context instead of
//! calling OpenHuman internals directly — the inversion that lets them live in
//! the standalone `tinychannels` crate. Lean providers ignore the host.

mod adapters;
mod channel_events;
pub(crate) mod remote_control;

pub use adapters::{
    ConfigAllowlistStore, ConversationHistoryStore, CoreApprovalGate, CoreShutdownRegistry,
    OpenHumanEventSink, VoiceSynthesizer, VoiceTranscriber,
};
pub use channel_events::{ChannelApprovalSurfaceSubscriber, ChannelTurnStateSubscriber};

use std::sync::Arc;

use tinychannels::host::ChannelHostBuilder;
use tinychannels::ChannelHost;

use crate::config::Config;

/// Assemble the full OpenHuman [`ChannelHost`] from a config snapshot.
///
/// Wires every capability OpenHuman can back today: lifecycle (shutdown),
/// STT, TTS, approval-reply parsing, conversation history, and the
/// web-channel event sink. Capabilities OpenHuman cannot yet express portably
/// (turn dispatch, run ledger, pairing) are simply left unset — a provider
/// that needs one degrades gracefully. Emoji reactions are deliberately unset:
/// the local-model reaction gate was removed, so providers never react.
pub fn build_channel_host(config: Arc<Config>) -> Arc<dyn ChannelHost> {
    ChannelHostBuilder::new()
        .lifecycle(Arc::new(CoreShutdownRegistry))
        .transcriber(Arc::new(VoiceTranscriber {
            config: Arc::clone(&config),
        }))
        .synthesizer(Arc::new(VoiceSynthesizer {
            config: Arc::clone(&config),
        }))
        .approvals(Arc::new(CoreApprovalGate))
        .conversations(Arc::new(ConversationHistoryStore {
            workspace_dir: config.workspace_dir.clone(),
        }))
        .events(Arc::new(OpenHumanEventSink))
        .allowlist(Arc::new(ConfigAllowlistStore))
        .build()
}

#[cfg(test)]
#[path = "host_tests.rs"]
mod tests;
