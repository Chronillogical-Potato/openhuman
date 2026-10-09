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
mod inference;
mod jev;
#[cfg(feature = "modules")]
mod search;

use crate::config::Config;
use std::time::{Duration, Instant};

pub(crate) use arm::Arm;

const TIMEOUT_ENV: &str = "OPENHUMAN_TINYMEMES_TIMEOUT_MS";
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(20);

/// Whether this thread's answer text must be held back from streaming because
/// its final reply will be remixed (treatment arm). The UI then shows only the
/// remixed reply instead of the original streaming in and being replaced.
pub(crate) fn holds_text_stream(thread_id: &str) -> bool {
    arm::assign(arm::mode(), thread_id) == Arm::Treatment
}

/// Remix a finished turn's reply. Returns the text to deliver instead, or
/// `None` to deliver the original. `turn_elapsed` is the agent turn's own
/// duration, logged for both arms so throughput can be compared.
pub(crate) async fn remix_final_reply(
    config: &Config,
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
    let Some(host) = host::host_for(config) else {
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
    let messages = crate::threads::store::get_messages(config.workspace_dir.clone(), thread_id)
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

    // Meme research, off the critical path: a meme was allowed but nothing in
    // the catalog fit. New GIFs are vetted before Jev can pick them.
    if outcome.wants_more_memes() {
        if let Some(reading) = &outcome.reading {
            let host = host.clone();
            let intent = reading.reply_intent;
            let request_id = request_id.to_owned();
            // Only a short, generic meme concept derived from this is searched.
            let moment = user_message.to_owned();
            tokio::spawn(async move {
                match host.engine.learn_memes(intent, &moment).await {
                    Ok(Some(r)) => log::info!(
                        "[tinymemes] meme research request_id={request_id} query={:?} found={} \
                         approved={} pending={} rejected_rating={} rejected_topic={} duplicates={} \
                         failed_jev={}",
                        r.query,
                        r.found,
                        r.approved,
                        r.pending,
                        r.rejected_rating,
                        r.rejected_topic,
                        r.duplicates,
                        r.failed_verification
                    ),
                    Ok(None) => {
                        log::debug!("[tinymemes] meme research skipped (repeat, budget, or busy)")
                    }
                    Err(e) => log::warn!("[tinymemes] meme research failed: {e}"),
                }
                host.save_memes();
            });
        }
    }

    let remixed = outcome.remix.is_some() && outcome.reply.trim() != reply.trim();
    let result = if remixed {
        host.mark_remixed(crate::threads::store::run_reply_message_id(request_id));
        host.save_index();
        host.save_memes();
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
         remix_ms={remix_ms} outcome={result} score={} tier={} mode={} memes={} rewrite_kept={} \
         dupes={} slang_enough={} wants_search={} meme_pick={} meme_p={}",
        rating.map_or(-1, |r| i32::from(r.score)),
        rating.map_or("none", |r| match r.tier {
            tinymemes::Tier::Off => "off",
            tinymemes::Tier::Light => "light",
            tinymemes::Tier::Spicy => "spicy",
            tinymemes::Tier::Unhinged => "unhinged",
        }),
        remix.map_or("none", |r| match r.mode {
            tinymemes::RemixMode::Rewrite => "rewrite",
            tinymemes::RemixMode::MemeOnly => "meme_only",
        }),
        remix.map_or(0, |r| r.memes.len()),
        remix.is_none_or(|r| r.rewrite_kept),
        remix.map_or(0, |r| r.duplicates_removed),
        reading
            .and_then(|r| r.slang_enough)
            .map_or_else(|| "none".to_owned(), |p| format!("{p:.2}")),
        reading.is_some_and(|r| r.wants_more_slang(0.5)),
        reading.map_or_else(
            || "none".to_owned(),
            |r| match &r.meme {
                tinymemes::reading::MemePick::Pick(t) => t.replace(' ', "_"),
                tinymemes::reading::MemePick::NoneFit => "none_fit".to_owned(),
                tinymemes::reading::MemePick::Unasked => "unasked".to_owned(),
            }
        ),
        reading
            .and_then(|r| r.meme_p)
            .map_or_else(|| "none".to_owned(), |p| format!("{p:.2}")),
    );
}
