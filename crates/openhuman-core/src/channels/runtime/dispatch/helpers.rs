//! OpenHuman-specific dispatch prompt context plus TinyChannels helpers.

use crate::channels::context::CHANNEL_TYPING_REFRESH_INTERVAL_SECS;
use crate::channels::traits;
use std::time::Duration;

/// Maximum characters shown in the debug reply println.
pub(super) const REPLY_LOG_TRUNCATE_CHARS: usize = 200;

pub(super) use tinychannels::runtime::{log_worker_join_result, select_acknowledgment_reaction};

/// Start the portable typing lifecycle with OpenHuman's configured interval.
pub(super) fn spawn_scoped_typing_task(
    channel: std::sync::Arc<dyn crate::channels::Channel>,
    recipient: String,
    cancellation_token: tokio_util::sync::CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tinychannels::runtime::spawn_scoped_typing_task(
        channel,
        recipient,
        cancellation_token,
        Duration::from_secs(CHANNEL_TYPING_REFRESH_INTERVAL_SECS),
    )
}

/// Build the OpenHuman `cron_add` delivery instruction for a channel turn.
pub(super) fn build_channel_context_block(msg: &traits::ChannelMessage) -> String {
    let channel = msg.channel.trim();
    if channel.is_empty()
        || channel.eq_ignore_ascii_case("web")
        || channel.eq_ignore_ascii_case("cli")
    {
        return String::new();
    }
    let reply_target = msg.reply_target.trim();
    if reply_target.is_empty() {
        return String::new();
    }
    format!(
        "[Channel context]\n\
         You are responding via the \"{channel}\" channel. Reply target: \"{reply_target}\".\n\
         For any cron/scheduled reminder you create with `cron_add`, set `delivery` to \
         `{{ \"mode\": \"announce\", \"channel\": \"{channel}\", \"to\": \"{reply_target}\" }}` \
         so the reminder is delivered back here instead of the in-app web stream. \
         Only fall back to the default proactive delivery if the user explicitly asks for \
         in-app/desktop notification.\n\n"
    )
}
