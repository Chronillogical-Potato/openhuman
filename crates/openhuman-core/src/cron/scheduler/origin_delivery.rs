//! `delivery.mode = "origin"`: commit a run's reply into the conversation that
//! asked for it and send it there once.
//!
//! - Empty output, or exactly [`NO_REPLY`], is an intentional "nothing to
//!   say": recorded as [`DeliveryStatus::Suppressed`], nothing is sent and no
//!   alert is raised.
//! - Web origin: the reply is stored in the origin thread first, under the
//!   deterministic id of `run_reply_message_id("cron:<job>:<run>")`, then
//!   announced on the web channel as a `proactive_message` event addressed to
//!   that thread. Re-delivering the same run lands on the same row.
//! - Channel origin: the reply is sent to the origin's reply target, then
//!   appended to that chat's history so its next turn sees it.

use super::delivery::strip_openhuman_link_markup;
use crate::config::Config;
use crate::cron::{channel_bridge, CronJob, DeliveryStatus, JobOrigin};
use crate::web_chat::{publish_web_channel_event, WebChannelEvent};
use anyhow::{anyhow, Result};
use std::path::Path;

/// What a scheduled run replies when it has nothing worth sending.
pub(crate) const NO_REPLY: &str = "NO_REPLY";

/// True when a run's output means "deliver nothing": blank, or exactly
/// [`NO_REPLY`] after trimming (case-sensitive).
pub(crate) fn is_suppressed_output(output: &str) -> bool {
    let trimmed = output.trim();
    trimmed.is_empty() || trimmed == NO_REPLY
}

/// The deterministic request id of one run's delivered reply.
pub(crate) fn origin_request_id(job_id: &str, run_id: &str) -> String {
    format!("cron:{job_id}:{run_id}")
}

/// Everything the web agent-transcript append needs about one delivered reply.
pub(crate) struct TranscriptAppend<'a> {
    pub thread_id: &'a str,
    pub agent_id: Option<&'a str>,
    pub text: &'a str,
    /// `cron:<job_id>:<run_id>`; the append must be idempotent on it.
    pub idempotency_key: &'a str,
    pub job_id: &'a str,
    pub run_id: &'a str,
}

/// SEAM: append the delivered reply to the origin thread's **agent transcript**
/// (the model-facing `SessionRef` transcript), so the next live turn in that
/// thread sees the reminder as an assistant message.
///
/// A no-op until tinyagents exposes a background-append API
/// (`append_background_message(session_ref, msg, idempotency_key, provenance)`:
/// serialized behind an active turn, generation-checked, deduped on the key,
/// provenance `{kind: "cron", job_id, run_id}`). The thread-store row the UI
/// reads is written separately by [`deliver_to_origin`].
pub(crate) async fn append_to_origin_transcript(
    _workspace_dir: &Path,
    append: &TranscriptAppend<'_>,
) -> Result<(), String> {
    tracing::debug!(
        job_id = %append.job_id,
        run_id = %append.run_id,
        has_agent_id = append.agent_id.is_some(),
        text_len = append.text.len(),
        "[cron] append_to_origin_transcript is a no-op until the tinyagents background-append API lands"
    );
    Ok(())
}

/// Deliver one run's output to the job's origin conversation.
///
/// `Ok(Suppressed)` for an intentional empty/`NO_REPLY` output; `Ok(Delivered)`
/// when the reply was committed and sent; `Err` when it could not be.
pub(crate) async fn deliver_to_origin(
    config: &Config,
    job: &CronJob,
    run_id: &str,
    output: &str,
) -> Result<DeliveryStatus> {
    if is_suppressed_output(output) {
        tracing::debug!(
            job_id = %job.id,
            run_id = %run_id,
            "[cron] origin delivery suppressed (empty or NO_REPLY)"
        );
        return Ok(DeliveryStatus::Suppressed);
    }
    let Some(origin) = job.origin.as_ref() else {
        return Err(anyhow!(
            "delivery mode 'origin' requires the job to have an origin conversation"
        ));
    };
    let text = output.trim();

    match origin {
        JobOrigin::Web {
            thread_id,
            agent_id,
        } => deliver_to_web_thread(config, job, run_id, thread_id, agent_id.as_deref(), text).await,
        JobOrigin::Channel {
            channel,
            reply_target,
            history_key,
            thread_id,
            ..
        } => {
            let text = strip_openhuman_link_markup(text);
            channel_bridge::send_to_channel(channel, reply_target, thread_id.as_deref(), &text)
                .await
                .map_err(|e| anyhow!(e))?;
            channel_bridge::append_assistant_message(history_key, &text);
            tracing::debug!(
                job_id = %job.id,
                run_id = %run_id,
                channel = %channel,
                "[cron] origin delivery sent to channel"
            );
            Ok(DeliveryStatus::Delivered)
        }
    }
}

async fn deliver_to_web_thread(
    config: &Config,
    job: &CronJob,
    run_id: &str,
    thread_id: &str,
    agent_id: Option<&str>,
    text: &str,
) -> Result<DeliveryStatus> {
    let request_id = origin_request_id(&job.id, run_id);

    // Store first: a reply that reached the announcement must already be on
    // disk, and a missing thread (deleted since the job was created) fails here
    // instead of announcing into nowhere.
    let workspace = config.workspace_dir.clone();
    let persist_thread = thread_id.to_string();
    let persist_request = request_id.clone();
    let persist_text = text.to_string();
    tokio::task::spawn_blocking(move || {
        crate::web_chat::persist_delivered_reply(
            &workspace,
            &persist_thread,
            &persist_request,
            &persist_text,
            &[],
        )
    })
    .await
    .map_err(|e| anyhow!("origin reply persistence task failed: {e}"))?
    .map_err(|e| anyhow!("could not store the reply in the origin thread: {e}"))?;

    if let Err(e) = append_to_origin_transcript(
        &config.workspace_dir,
        &TranscriptAppend {
            thread_id,
            agent_id,
            text,
            idempotency_key: &request_id,
            job_id: &job.id,
            run_id,
        },
    )
    .await
    {
        tracing::warn!(
            job_id = %job.id,
            run_id = %run_id,
            error = %e,
            "[cron] origin transcript append failed; thread row is stored"
        );
    }

    publish_web_channel_event(WebChannelEvent {
        event: "proactive_message".to_string(),
        client_id: "system".to_string(),
        thread_id: thread_id.to_string(),
        request_id: request_id.clone(),
        full_response: Some(text.to_string()),
        success: Some(true),
        ..Default::default()
    });
    tracing::debug!(
        job_id = %job.id,
        run_id = %run_id,
        request_id = %request_id,
        "[cron] origin delivery stored and announced on web thread"
    );
    Ok(DeliveryStatus::Delivered)
}

#[cfg(test)]
#[path = "origin_delivery_tests.rs"]
mod tests;
