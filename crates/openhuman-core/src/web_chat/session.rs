//! Builds and fingerprints the cached session `Agent`: target agent
//! resolution, model-override normalization, locale reply directive, and the
//! `SessionCacheFingerprint` that decides whether a cached agent can be
//! reused for the next turn on a thread.

use crate::agent::OpenHumanSessionHost;
use crate::config::Config;
use serde_json::json;

use super::types::{SessionCacheFingerprint, SessionEntry};

pub(super) fn autonomy_signature(config: &Config) -> String {
    serde_json::to_string(&config.autonomy).unwrap_or_default()
}

/// Signature of `config.model_registry` for the session-cache fingerprint.
/// Captures every per-model `vision` flag so toggling one in Settings forces a
/// rebuild (picking up the new build-time `model_vision`). Mirrors
/// [`autonomy_signature`].
pub(super) fn model_registry_signature(config: &Config) -> String {
    serde_json::to_string(&config.model_registry).unwrap_or_default()
}

pub(super) fn pick_target_agent_id(_config: &Config) -> String {
    "orchestrator".to_string()
}

pub(crate) fn normalize_model_override(model_override: Option<String>) -> Option<String> {
    model_override
        .map(|model| model.trim().to_string())
        .filter(|model| !model.is_empty())
}

pub(crate) fn provider_role_for_model_override(model_override: Option<&str>) -> &'static str {
    // A role alias (`hint:coding`) or a retired tier slug (`hint:coding`) picks
    // that workload's route; a concrete model id rides the chat route.
    match model_override.map(str::trim) {
        Some(value) if value.starts_with("hint:") || crate::config::is_legacy_tier_model(value) => {
            match crate::inference::provider::factory::role_for_model_tier(value) {
                role @ ("agentic" | "coding" | "summarization" | "reasoning") => role,
                _ => "chat",
            }
        }
        _ => "chat",
    }
}

pub(super) fn build_session_agent(
    config: &Config,
    client_id: &str,
    thread_id: &str,
    target_agent_id: &str,
    model_override: Option<String>,
    temperature: Option<f64>,
    locale: Option<&str>,
) -> Result<OpenHumanSessionHost, String> {
    let mut effective = config.clone();
    if let Some(model) = model_override {
        effective.default_model = Some(model);
    }
    let provider_role = provider_role_for_model_override(effective.default_model.as_deref());
    if let Some(temp) = temperature {
        effective.default_temperature = temp;
    }

    log::info!(
        "[web-channel] routing chat turn to '{}' provider_role='{}' (client_id={}, thread_id={})",
        target_agent_id,
        provider_role,
        client_id,
        thread_id
    );

    let locale_directive = locale.and_then(locale_reply_directive);
    if let Some(s) = locale_directive.as_deref() {
        log::info!(
            "[web-channel] injecting locale directive client={} thread={} locale={} directive={:?}",
            client_id,
            thread_id,
            locale.unwrap_or(""),
            s
        );
    }

    let agent_result = OpenHumanSessionHost::from_config_for_agent(&effective, target_agent_id);

    agent_result
        .map(|mut agent| {
            agent.set_event_context(
                json!({"client_id": client_id, "thread_id": thread_id}).to_string(),
                "web_channel",
            );
            let short_thread = if thread_id.len() > 12 {
                &thread_id[..12]
            } else {
                thread_id
            };
            agent.set_agent_definition_name(format!("{target_agent_id}_{short_thread}"));
            agent
        })
        .map_err(|e| e.to_string())
}

pub(crate) fn locale_reply_directive(locale: &str) -> Option<String> {
    let language = match locale.trim() {
        "ar" => "Arabic",
        "bn" => "Bengali",
        "es" => "Spanish",
        "fr" => "French",
        "hi" => "Hindi",
        "id" => "Indonesian",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "zh-CN" | "zh" => "Simplified Chinese",
        _ => return None,
    };
    Some(format!(
        "User language: the user's interface is set to {language}. \
         Respond in {language} unless the user explicitly asks for a different language. \
         Keep proper nouns, code, and command names untranslated."
    ))
}

pub(super) fn build_session_fingerprint(
    config: &Config,
    model_override: Option<String>,
    temperature: Option<f64>,
    target_agent_id: String,
    provider_role: &str,
) -> SessionCacheFingerprint {
    SessionCacheFingerprint {
        model_override,
        temperature,
        provider_binding: crate::inference::provider::provider_for_role(provider_role, config),
        target_agent_id,
        autonomy_signature: autonomy_signature(config),
        model_registry_signature: model_registry_signature(config),
    }
}

/// How `checkout_session_agent` treats the thread's cached entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CheckoutPolicy {
    /// A user turn: reuse the cached agent only when its
    /// `SessionCacheFingerprint` matches this turn's model/temperature/agent,
    /// otherwise rebuild (and cold-boot resume) with the new settings.
    Exact,
    /// A host-authored turn: reuse whatever agent the thread has, under the
    /// settings the user's last turn chose, and hand it back with that same
    /// fingerprint. The turn has no settings of its own, and rebuilding on a
    /// mismatch would evict the warm session for nothing.
    AdoptCached,
    /// A parallel fork: never take or return the cached agent; build fresh from
    /// the thread's durable history.
    Fork,
}

/// A session agent checked out of the per-thread cache for exactly one turn.
///
/// Every turn that runs on a conversation thread — a user turn, a
/// background-delivery turn, a goal continuation — must go through the same
/// checkout so it appends to the thread's live history and its transcript.
/// A turn run on a throwaway `OpenHumanSessionHost` bound to the thread writes
/// a *second* root transcript for that thread with a newer `created`, which the
/// next cold-boot resume then prefers over the real one — dropping every turn
/// the user had with the cached session (the "20–30 days" amnesia).
pub(crate) struct CheckedOutSession {
    pub(crate) agent: OpenHumanSessionHost,
    pub(crate) fingerprint: SessionCacheFingerprint,
}

/// Take the thread's cached session agent, or build one and cold-boot resume
/// it from the thread's durable history.
///
/// The entry is *removed* from the cache for the duration of the turn so two
/// turns can never drive one agent; `checkin_session_agent` /
/// `checkin_session_agent_if_vacant` put it back.
pub(crate) async fn checkout_session_agent(
    config: &Config,
    client_id: &str,
    thread_id: &str,
    model_override: Option<String>,
    temperature: Option<f64>,
    locale: Option<&str>,
    policy: CheckoutPolicy,
    // The message this turn is about to send, so a cold-boot seed from the
    // conversation log can drop it when the client already stored it. Empty for
    // a host-authored turn, whose notice is never in the store.
    current_user_message: &str,
) -> Result<CheckedOutSession, String> {
    let map_key = super::ops::key_for(thread_id);
    let target_agent_id = pick_target_agent_id(config);
    let provider_role = provider_role_for_model_override(model_override.as_deref());
    let fingerprint = build_session_fingerprint(
        config,
        model_override.clone(),
        temperature,
        target_agent_id.clone(),
        provider_role,
    );

    // A forked (parallel) turn never reuses or evicts the shared cached agent —
    // it always builds fresh from the history snapshot below.
    let prior = if policy == CheckoutPolicy::Fork {
        None
    } else {
        let mut sessions = super::ops::THREAD_SESSIONS.lock().await;
        sessions.remove(&map_key)
    };

    let (mut agent, fingerprint, was_built_fresh) = match prior {
        Some(entry)
            if entry.fingerprint == fingerprint || policy == CheckoutPolicy::AdoptCached =>
        {
            log::info!(
                "[web-channel] reusing cached session agent id={} for client={} thread={}",
                entry.fingerprint.target_agent_id,
                client_id,
                thread_id
            );
            (entry.agent, entry.fingerprint, false)
        }
        Some(prior_entry) => {
            log::info!(
                "[web-channel] cache miss — rebuilding session agent \
                 (was id={}, now id={}; prior_provider_binding={}, now={}) \
                 for client={} thread={}",
                prior_entry.fingerprint.target_agent_id,
                target_agent_id,
                prior_entry.fingerprint.provider_binding,
                fingerprint.provider_binding,
                client_id,
                thread_id
            );
            (
                build_session_agent(
                    config,
                    client_id,
                    thread_id,
                    &target_agent_id,
                    model_override,
                    temperature,
                    locale,
                )?,
                fingerprint,
                true,
            )
        }
        None => (
            build_session_agent(
                config,
                client_id,
                thread_id,
                &target_agent_id,
                model_override,
                temperature,
                locale,
            )?,
            fingerprint,
            true,
        ),
    };

    // Cold-boot resume needs no seeding here. `set_thread_id` binds the
    // session's durable identity and the turn resumes by it, reading the one
    // transcript this conversation has ever had — tool calls, tool results and
    // reasoning included. The old path seeded by hand from whichever root
    // transcript matched the thread and newest, and fell back to the
    // conversation log's prose pairs when that failed; the prose fallback also
    // carried no system message, so such a turn reached the provider with no
    // system prompt and no prompt-cache key at all.
    let _ = (config, current_user_message);

    Ok(CheckedOutSession { agent, fingerprint })
}

/// Return a checked-out agent to the thread cache, replacing whatever is there.
/// The primary user-turn path: it owns the thread's `IN_FLIGHT` slot, so any
/// entry it finds was left by a turn that ran concurrently and is now stale.
pub(crate) async fn checkin_session_agent(
    thread_id: &str,
    agent: OpenHumanSessionHost,
    fingerprint: SessionCacheFingerprint,
) {
    let mut sessions = super::ops::THREAD_SESSIONS.lock().await;
    sessions.insert(
        super::ops::key_for(thread_id),
        SessionEntry { agent, fingerprint },
    );
}

/// Return a checked-out agent to the thread cache only when the slot is still
/// empty. Host-authored turns (background delivery, goal continuation) do not
/// hold `IN_FLIGHT`, so a user turn that started while they ran built its own
/// agent and cached it; that one carries the user's newer turn and must win.
/// Both turns appended to the thread's durable transcript regardless.
pub(crate) async fn checkin_session_agent_if_vacant(
    thread_id: &str,
    agent: OpenHumanSessionHost,
    fingerprint: SessionCacheFingerprint,
) -> bool {
    let mut sessions = super::ops::THREAD_SESSIONS.lock().await;
    match sessions.entry(super::ops::key_for(thread_id)) {
        std::collections::hash_map::Entry::Occupied(_) => {
            log::info!(
                "[web-channel] system turn finished after a newer turn re-cached thread={} — \
                 dropping the system turn's agent",
                thread_id
            );
            false
        }
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(SessionEntry { agent, fingerprint });
            true
        }
    }
}

#[cfg(test)]
#[path = "session_checkout_tests.rs"]
mod session_checkout_tests;
