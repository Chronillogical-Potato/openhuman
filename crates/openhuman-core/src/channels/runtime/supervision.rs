//! OpenHuman observer adapter for TinyChannels listener supervision.

use super::super::traits;
use super::super::Channel;
use crate::core::bus::BUS;
use crate::core::events::DomainEvent;
use std::sync::Arc;
use tinychannels::runtime::ListenerObserver;

/// Publishes reusable listener lifecycle facts onto OpenHuman's domain bus.
struct OpenHumanListenerObserver;

impl ListenerObserver for OpenHumanListenerObserver {
    fn connected(&self, channel: &str) {
        BUS.publish(DomainEvent::ChannelConnected {
            channel: channel.to_string(),
        });
    }

    fn disconnected(&self, channel: &str, reason: &str, failed: bool) {
        if failed {
            let message = format!("Channel {channel} error: {reason}; restarting");
            crate::core::observability::report_error_or_expected(
                &message,
                "channels",
                "supervised_listener",
                &[("channel", channel)],
            );
        } else {
            tracing::warn!("Channel {channel} exited unexpectedly; restarting");
        }
        BUS.publish(DomainEvent::ChannelDisconnected {
            channel: channel.to_string(),
            reason: reason.to_string(),
        });
    }

    fn restarted(&self, channel: &str) {
        BUS.publish(DomainEvent::HealthRestarted {
            component: format!("channel:{channel}"),
        });
    }
}

/// Start a TinyChannels listener with OpenHuman observability attached.
pub(crate) fn spawn_supervised_listener(
    channel: Arc<dyn Channel>,
    tx: tokio::sync::mpsc::Sender<traits::ChannelMessage>,
    initial_backoff_secs: u64,
    max_backoff_secs: u64,
) -> tokio::task::JoinHandle<()> {
    crate::platform::health::bus::register_health_subscriber();
    tinychannels::runtime::spawn_supervised_listener(
        channel,
        tx,
        initial_backoff_secs,
        max_backoff_secs,
        Arc::new(OpenHumanListenerObserver),
    )
}

#[cfg(test)]
pub(crate) use tinychannels::runtime::{jitter_millis, MAX_JITTER_MS};

#[cfg(test)]
#[path = "supervision_tests.rs"]
mod tests;
