//! Sentry client options with OpenHuman's single `before_send` chain.
//!
//! The desktop shell and the CLI each grew their own `before_send` filter
//! and the two drifted (12 vs 19 predicates). This is the one chain: the
//! union of both, in the CLI's order with the shell's dev-server filter in
//! front, followed by hostname stripping, the user-id fallback and secret
//! scrubbing. What differs per host — DSN, release, environment and where a
//! fallback user id comes from — is a [`SentryConfig`] field; the panic hook
//! and scope tags stay with the host.

use std::borrow::Cow;
use std::sync::Arc;

use openhuman_core::core::observability as obs;
pub use sentry::protocol::Event;
pub use sentry::ClientOptions;

/// Re-exported so a host initialises the same `sentry` the options were
/// built for.
pub use sentry as sdk;

/// Where a host finds the signed-in user's id when the Sentry scope has not
/// bound one (pre-login or pre-boot events).
pub type UserIdSource = fn() -> Option<String>;

/// The per-host inputs to [`client_options`].
#[derive(Clone)]
pub struct SentryConfig {
    /// The DSN; `None` (or unparsable) gives a client that sends nothing.
    pub dsn: Option<String>,
    /// The release tag; see [`release_tag`].
    pub release: String,
    /// The deployment environment (`production`, `staging`, ...).
    pub environment: String,
    /// The fallback user-id source; defaults to [`credential_user_id`].
    pub user_id: UserIdSource,
}

impl SentryConfig {
    /// A config with the credential-store user-id fallback.
    pub fn new(dsn: Option<String>, release: String, environment: String) -> Self {
        Self {
            dsn,
            release,
            environment,
            user_id: credential_user_id,
        }
    }
}

/// The canonical release tag, `openhuman@<version>[+<sha12>]`, matching the
/// frontend's `SENTRY_RELEASE` so every surface groups under one release.
pub fn release_tag(version: &str, build_sha: Option<&str>) -> String {
    let sha: String = build_sha.unwrap_or("").trim().chars().take(12).collect();
    if sha.is_empty() {
        format!("openhuman@{version}")
    } else {
        format!("openhuman@{version}+{sha}")
    }
}

/// The first non-blank value among `candidates` (runtime env values first,
/// then compile-time ones, as the hosts resolve their DSN).
pub fn first_non_blank(candidates: impl IntoIterator<Item = Option<String>>) -> Option<String> {
    candidates
        .into_iter()
        .flatten()
        .map(|value| value.trim().to_string())
        .find(|value| !value.is_empty())
}

/// The signed-in user's id from the core's credential identity slot — the
/// CLI's fallback, and the default for [`SentryConfig::user_id`].
pub fn credential_user_id() -> Option<String> {
    openhuman_core::security::credentials::identity::peek_credential_user_identity()
        .and_then(|identity| identity.id)
}

/// Sentry options with the shared filter chain, secret scrubbing, PII off,
/// and the core's shared-reqwest transport.
pub fn client_options(config: SentryConfig) -> ClientOptions {
    let user_id = config.user_id;
    log::debug!(
        "[embed][sentry] client options release={} environment={} dsn_set={}",
        config.release,
        config.environment,
        config.dsn.is_some()
    );
    ClientOptions {
        dsn: config.dsn.as_deref().and_then(|dsn| dsn.parse().ok()),
        release: Some(Cow::Owned(config.release)),
        environment: Some(Cow::Owned(config.environment)),
        send_default_pii: false,
        before_send: Some(Arc::new(move |event| before_send(event, user_id))),
        sample_rate: 1.0,
        transport: Some(Arc::new(openhuman_core::core::sentry_transport::factory)),
        ..ClientOptions::default()
    }
}

/// One named predicate of the chain.
type NoiseFilter = (&'static str, fn(&Event<'static>) -> bool);

/// The chain, in evaluation order. Each name is the grep-able tag in the
/// drop log line. Rationale for each lives on the core predicate.
const NOISE_FILTERS: &[NoiseFilter] = &[
    ("localhost-dev-fetch", is_localhost_dev_fetch_noise),
    (
        "transient-provider-http",
        obs::is_transient_provider_http_failure,
    ),
    (
        "provider-exhaustion",
        obs::is_all_transient_provider_exhaustion_event,
    ),
    ("backend-error-code", obs::is_backend_error_code_event),
    (
        "provider-transport",
        obs::is_transient_provider_transport_failure,
    ),
    ("budget", obs::is_budget_event),
    ("insufficient-credits", obs::is_insufficient_credits_event),
    ("quota-exhausted", obs::is_quota_exhausted_event),
    ("ollama-cloud-500", obs::is_ollama_cloud_internal_500_event),
    (
        "fs-limitation",
        obs::is_windows_file_system_limitation_event,
    ),
    ("max-iterations", obs::is_max_iterations_event),
    (
        "transient-backend-api",
        obs::is_transient_backend_api_failure,
    ),
    (
        "transient-integrations",
        obs::is_transient_integrations_failure,
    ),
    ("updater-transient", obs::is_updater_transient_event),
    (
        "skill-install-fetch",
        obs::is_skill_install_user_fetch_failure,
    ),
    (
        "skills-install-4xx",
        obs::is_skills_install_client_error_event,
    ),
    (
        "channel-message-404",
        obs::is_channel_message_not_found_event,
    ),
    ("user-config-provider", obs::is_user_config_provider_event),
    ("connectivity", obs::is_connectivity_event),
    ("stale-release", obs::is_stale_release_event),
    ("session-expired", obs::is_session_expired_event),
];

/// The name of the first filter that classifies `event` as known noise.
pub fn known_noise(event: &Event<'static>) -> Option<&'static str> {
    NOISE_FILTERS
        .iter()
        .find(|(_, matches)| matches(event))
        .map(|(name, _)| *name)
}

/// The `before_send` hook: drop known noise, otherwise strip the hostname,
/// fill a missing user id from `user_id`, and scrub secrets from the message
/// and exception values.
pub fn before_send(mut event: Event<'static>, user_id: UserIdSource) -> Option<Event<'static>> {
    if let Some(filter) = known_noise(&event) {
        // Metadata only: the message can carry backend bodies and tokens.
        log::debug!(
            "[sentry-filter] dropping {filter} event_id={:?}",
            event.event_id
        );
        return None;
    }
    event.server_name = None;
    if event.user.is_none() {
        event.user = user_id().map(|id| sentry::User {
            id: Some(id),
            ..Default::default()
        });
    }
    for exception in &mut event.exception.values {
        if let Some(value) = exception.value.as_deref() {
            exception.value = Some(scrub_secrets(value));
        }
    }
    if let Some(message) = event.message.take() {
        event.message = Some(scrub_secrets(&message));
    }
    Some(event)
}

/// The shared secret scrubber the chain applies.
pub fn scrub_secrets(input: &str) -> String {
    openhuman_core::core::log_redaction::scrub_secrets(input)
}

/// A webview asking a dev server (`http://localhost:1420`) that is not
/// running in a packaged build: `Failed to request http://localhost:…`.
/// From the desktop shell's chain (OPENHUMAN-TAURI-V).
fn is_localhost_dev_fetch_noise(event: &Event<'static>) -> bool {
    let direct = event.message.as_deref();
    let from_exception = event.exception.last().and_then(|e| e.value.as_deref());
    [direct, from_exception]
        .into_iter()
        .flatten()
        .any(message_is_localhost_dev_fetch_noise)
}

/// The prefix rule behind [`is_localhost_dev_fetch_noise`].
pub fn message_is_localhost_dev_fetch_noise(message: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "Failed to request http://localhost:",
        "Failed to request http://127.0.0.1:",
    ];
    PREFIXES.iter().any(|prefix| message.starts_with(prefix))
}

#[cfg(test)]
#[path = "process_sentry_tests.rs"]
mod tests;
