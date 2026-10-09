//! TinyMemes host adapter: remixes the orchestrator's final reply with
//! region slang and memes when the chat can take it.
//!
//! - Gated by `OPENHUMAN_TINYMEMES` (off by default; see [`arm`] for `on` and
//!   the `ab` / `ab:NN` experiment split).
//! - Runs after the turn, on `full_response` only. Sub-agent output never
//!   reaches it.
//! - Reads the thread as the user saw it (earlier replies already remixed),
//!   not the agent's original transcript. The agent's own transcript keeps the
//!   original wording, so the orchestrator never sees its replies in slang.
//! - Fails open: any error or timeout delivers the original reply.
//! - Slang web research runs in the background after delivery, only when Jev
//!   judges the slang index short for the reply.
//!
//! Every turn in an experiment logs one grep-friendly `[tinymemes] turn` line
//! (arm, turn time, remix time, rating, outcome) for the A/B comparison. No
//! user content is logged.

mod arm;
mod host;
mod key;

use std::path::Path;
use std::time::{Duration, Instant};

pub(crate) use arm::Arm;

const TIMEOUT_ENV: &str = "OPENHUMAN_TINYMEMES_TIMEOUT_MS";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

/// Remix a finished turn's reply. Returns the text to deliver instead, or
/// `None` to deliver the original. `turn_elapsed` is the agent turn's own
/// duration, logged for both arms so throughput can be compared.
pub(crate) async fn remix_final_reply(
    workspace_dir: &Path,
    thread_id: &str,
    request_id: &str,
    user_message: &str,
    reply: &str,
    turn_elapsed: Duration,
) -> Option<String> {
    let arm = arm::assign(arm::mode(), thread_id);
    let bucket = arm::bucket(thread_id);
    let turn_ms = turn_elapsed.as_millis();
    match arm {
        Arm::Disabled => return None,
        Arm::Control => {
            log::info!(
                "[tinymemes] turn arm=control bucket={bucket} request_id={request_id} \
                 turn_ms={turn_ms} remix_ms=0 outcome=not_remixed"
            );
            return None;
        }
        Arm::Treatment => {}
    }

    let started = Instant::now();
    let Some(host) = host::host_for(workspace_dir) else {
        log_outcome(
            request_id,
            bucket,
            turn_ms,
            started,
            "engine_unavailable",
            None,
        );
        return None;
    };
    let messages = crate::threads::store::get_messages(workspace_dir.to_path_buf(), thread_id)
        .unwrap_or_else(|e| {
            log::debug!("[tinymemes] thread history unavailable: {e}");
            Vec::new()
        });
    let turns = host::history_turns(&messages, user_message, |id| host.is_remixed(id));

    let budget = std::env::var(TIMEOUT_ENV)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map(Duration::from_millis)
        .unwrap_or(DEFAULT_TIMEOUT);
    let outcome = match tokio::time::timeout(budget, host.engine.process(&turns, reply)).await {
        Ok(outcome) => outcome,
        Err(_) => {
            log_outcome(request_id, bucket, turn_ms, started, "timeout", None);
            return None;
        }
    };

    // Slang research, off the critical path, only for a reply that was
    // remixed and only when Jev judged the index short of slang for it.
    if let (Some(reading), Some(_)) = (&outcome.reading, &outcome.remix) {
        let policy = host.engine.slang_index().policy();
        if reading.wants_more_slang(policy.search_below) {
            let host = host.clone();
            let intent = reading.reply_intent;
            let reply = reply.to_owned();
            let request_id = request_id.to_owned();
            tokio::spawn(async move {
                match host.engine.learn_for_reply(intent, &reply).await {
                    Ok(Some(r)) => log::info!(
                        "[tinymemes] slang research request_id={request_id} added={} \
                         corroborated={} rejected={} failed_jev={}",
                        r.added,
                        r.corroborated,
                        r.rejected,
                        r.failed_verification
                    ),
                    Ok(None) => {
                        log::debug!("[tinymemes] slang research skipped (repeat or budget)")
                    }
                    Err(e) => log::warn!("[tinymemes] slang research failed: {e}"),
                }
                host.save_index();
            });
        }
    }

    let remixed = outcome.remix.is_some() && outcome.reply.trim() != reply.trim();
    let result = if remixed {
        host.mark_remixed(crate::threads::store::run_reply_message_id(request_id));
        host.save_index();
        "remixed"
    } else if outcome.skipped.is_some() && outcome.rating.is_none() {
        "error"
    } else {
        "unchanged"
    };
    log_outcome(request_id, bucket, turn_ms, started, result, Some(&outcome));
    remixed.then_some(outcome.reply)
}

fn log_outcome(
    request_id: &str,
    bucket: u8,
    turn_ms: u128,
    started: Instant,
    result: &str,
    outcome: Option<&tinymemes::Outcome>,
) {
    let remix_ms = started.elapsed().as_millis();
    let rating = outcome.and_then(|o| o.rating);
    let remix = outcome.and_then(|o| o.remix.as_ref());
    let reading = outcome.and_then(|o| o.reading.as_ref());
    log::info!(
        "[tinymemes] turn arm=treatment bucket={bucket} request_id={request_id} turn_ms={turn_ms} \
         remix_ms={remix_ms} outcome={result} score={} tier={} memes={} rewrite_kept={} \
         slang_enough={} wants_search={}",
        rating.map_or(-1, |r| i32::from(r.score)),
        rating.map_or("none", |r| match r.tier {
            tinymemes::Tier::Off => "off",
            tinymemes::Tier::Light => "light",
            tinymemes::Tier::Spicy => "spicy",
            tinymemes::Tier::Unhinged => "unhinged",
        }),
        remix.map_or(0, |r| r.memes.len()),
        remix.is_none_or(|r| r.rewrite_kept),
        reading
            .and_then(|r| r.slang_enough)
            .map_or_else(|| "none".to_owned(), |p| format!("{p:.2}")),
        reading.is_some_and(|r| r.wants_more_slang(0.5)),
    );
}
