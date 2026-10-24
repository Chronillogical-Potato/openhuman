use crate::security::secret_storage::SecretStore;
use crate::util::retry_with_backoff;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[allow(unused_imports)]
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[cfg(test)]
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[cfg(test)]
use std::sync::Arc;

mod keychain;
mod lock;
mod migration;
mod persistence;
use lock::{annotate_lock_create_failure, is_lock_create_unwritable_fs};
#[cfg(test)]
use lock::{in_process_lock_for, is_pid_alive};
#[cfg(test)]
use persistence::write_owner_only;
pub(crate) use persistence::is_persisted_document;
use persistence::{PersistedAuthProfile, PersistedAuthProfiles};
mod readonly;
pub(crate) use persistence::STORAGE_SECRET_NAME as PROFILES_SECRET_NAME;
mod store_core;
pub use readonly::ReadOnlyAuthProfileError;

pub(crate) fn load_profile_read_only(
    config: &crate::config::Config,
    provider: &str,
    profile_override: Option<&str>,
) -> Result<Option<AuthProfile>, ReadOnlyAuthProfileError> {
    readonly::load_profile(config, provider, profile_override)
}

const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Compact secret payload stored as a single keychain entry per auth profile.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct KeychainSecrets {
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub access_token: Option<String>,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
}
pub(crate) const PROFILES_FILENAME: &str = "auth-profiles.json";
/// The auth-profiles file a store over `state_dir` keeps.
pub(crate) fn profiles_file(state_dir: &Path) -> PathBuf {
    state_dir.join(PROFILES_FILENAME)
}
const LOCK_FILENAME: &str = "auth-profiles.lock";
const LOCK_WAIT_MS: u64 = 50;
/// A lock file that has existed for longer than this is treated as leaked
/// (its owner crashed without unlinking it, or `fs::remove_file` in the
/// guard's `Drop` was rejected by Windows AV/indexer and the file got
/// orphaned with the still-alive owner's pid in it). No legitimate
/// auth-profile operation holds the lock for anywhere near this long —
/// load+save is a tiny JSON read followed by an atomic rename. The
/// threshold is intentionally well above any realistic operation time
/// so we never reclaim under a slow-but-legitimate holder.
const STALE_LOCK_AGE_MS: u64 = 30_000;
/// Staleness threshold for a **malformed** lock — one with no parseable
/// `pid=` line. A healthy holder writes its pid microseconds after the
/// `create_new` succeeds, so a pidless lock older than this can only be a
/// crash/kill that landed between `create_new` and the `pid=` write (or an
/// abandoned in-flight writer). It is never a live, well-behaved holder, so
/// we reclaim it after a short grace instead of making every reader wait the
/// full [`STALE_LOCK_AGE_MS`]. This is what was leaving users stuck on
/// "Initializing OpenHuman" for ~30s after a kill+reopen: `app_state_snapshot`
/// → `load_app_session_profile` → `acquire_lock` blocked on a fresh pidless
/// lock. The grace is generous enough to never reclaim under a live writer
/// mid-`create_new`/`pid=` window (microseconds in practice).
const MALFORMED_LOCK_GRACE_MS: u64 = 2_000;
/// Wait long enough for a fresh leaked lock to cross the stale threshold
/// and be reclaimed before surfacing a lock timeout to the caller.
const LOCK_TIMEOUT_MS: u64 = STALE_LOCK_AGE_MS + 5_000;

/// Retry budget for the JSON write + rename in `write_persisted_locked`.
/// Same shape as the lock-create call at the bottom of `acquire_lock` (which
/// is what closed Sentry OPENHUMAN-TAURI-H1 / H8 in #1641 / #2085). With
/// `attempts = 6`, `retry_with_backoff` issues at most 6 calls and sleeps
/// 5 times between them (last failure breaks without sleeping):
/// `100+200+400+800+1600 ≈ 3.1s per stage`, so the write and rename stages
/// together sit at `≈6.2s` worst case. Sized to stay well inside
/// `LOCK_TIMEOUT_MS = 35_000` so concurrent acquire_lock callers never time
/// out behind a single retry-loop owner.
const PERSIST_RETRY_ATTEMPTS: u32 = 6;
const PERSIST_RETRY_BASE_MS: u64 = 100;

type EncryptedProfileFields = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthProfileKind {
    OAuth,
    Token,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenSet {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub id_token: Option<String>,
    #[serde(default)]
    pub expires_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

impl TokenSet {
    pub fn is_expiring_within(&self, skew: Duration) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let now_plus_skew =
                    Utc::now() + chrono::Duration::from_std(skew).unwrap_or_default();
                expires_at <= now_plus_skew
            }
            None => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfile {
    pub id: String,
    pub provider: String,
    pub profile_name: String,
    pub kind: AuthProfileKind,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub token_set: Option<TokenSet>,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl AuthProfile {
    pub fn new_oauth(provider: &str, profile_name: &str, token_set: TokenSet) -> Self {
        let now = Utc::now();
        let id = profile_id(provider, profile_name);
        Self {
            id,
            provider: provider.to_string(),
            profile_name: profile_name.to_string(),
            kind: AuthProfileKind::OAuth,
            account_id: None,
            workspace_id: None,
            token_set: Some(token_set),
            token: None,
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn new_token(provider: &str, profile_name: &str, token: String) -> Self {
        let now = Utc::now();
        let id = profile_id(provider, profile_name);
        Self {
            id,
            provider: provider.to_string(),
            profile_name: profile_name.to_string(),
            kind: AuthProfileKind::Token,
            account_id: None,
            workspace_id: None,
            token_set: None,
            token: Some(token),
            metadata: BTreeMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthProfilesData {
    pub schema_version: u32,
    pub updated_at: DateTime<Utc>,
    pub active_profiles: BTreeMap<String, String>,
    pub profiles: BTreeMap<String, AuthProfile>,
}

impl Default for AuthProfilesData {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            updated_at: Utc::now(),
            active_profiles: BTreeMap::new(),
            profiles: BTreeMap::new(),
        }
    }
}

/// Prefix used for keychain entries that store auth profile secrets.
/// Full key format (as handled by the keyring module): `"{user_id}:auth:{profile_id}"`.
const KEYCHAIN_AUTH_PREFIX: &str = "auth:";

/// The logical key used for an auth profile's secret payload.
pub(crate) fn auth_profile_secret_key(profile_id: &str) -> String {
    format!("{KEYCHAIN_AUTH_PREFIX}{profile_id}")
}

/// Derive a stable keychain user-id from a state directory path.
///
/// For a typical path like `/home/alice/.openhuman/users/uid-123` this
/// returns `"uid-123"`.  Falls back to a hash of the full path string so
/// the function always returns a non-empty value even for unusual layouts.
fn user_id_from_state_dir(state_dir: &Path) -> String {
    // The user directory is `{root}/users/{user_id}/` — take the last component.
    if let Some(id) = state_dir.file_name().and_then(|s| s.to_str()) {
        if !id.is_empty() {
            return id.to_string();
        }
    }
    // Fallback: use a hex hash of the path so we always get a stable string.
    let path_str = state_dir.to_string_lossy();
    let mut hash: u64 = 14695981039346656037u64; // FNV-1a offset basis
    for b in path_str.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(1099511628211u64);
    }
    format!("path-{hash:016x}")
}

#[derive(Debug, Clone)]
pub struct AuthProfilesStore {
    path: PathBuf,
    lock_path: PathBuf,
    secret_store: SecretStore,
    /// Opaque user identifier used to namespace keychain entries.
    user_id: String,
    /// Whether the OS keychain is available on this machine.
    /// Cached at construction time to avoid repeated probes.
    use_keychain: bool,
    /// `#[cfg(test)]` failure injection for the **write** stage of
    /// `write_persisted_locked`. When non-zero, the next call inside the
    /// `fs::write(tmp)` retry loop consumes one count and returns a
    /// `__TEST_TRANSIENT__` error so `is_transient_fs_error` treats it as
    /// retryable (`crates/openhuman-core/src/util.rs:618`). Production binaries never
    /// see this field.
    #[cfg(test)]
    force_transient_failures_write: Arc<AtomicUsize>,
    /// `#[cfg(test)]` failure injection for the **rename** stage of
    /// `write_persisted_locked`. Separate counter from the write stage so a
    /// test can exercise the rename retry loop without first having to drain
    /// failures through the write stage (see PR #3364 review feedback —
    /// the headline retry path was line-covered but not behaviour-covered
    /// before this split).
    #[cfg(test)]
    force_transient_failures_rename: Arc<AtomicUsize>,
    /// `#[cfg(test)]` failure injection — when set, the next `acquire_lock`
    /// call consumes the flag and returns a synthetic `StorageFull`
    /// lock-create failure, exercising the lock-free read-only fallback in
    /// [`AuthProfilesStore::load`] (Sentry TAURI-RUST-4SZ). Production
    /// binaries never see this field.
    #[cfg(test)]
    force_lock_unwritable: Arc<AtomicBool>,
}
/// Decrement an `AtomicUsize` failure-injection counter by one if it is
/// non-zero, returning a `__TEST_TRANSIENT__` error so `is_transient_fs_error`
/// classifies the failure as retryable. Used by both per-stage consumers in
/// `write_persisted_locked` (test-only).
#[cfg(test)]
fn consume_one(counter: &AtomicUsize) -> Result<()> {
    if counter.load(Ordering::SeqCst) == 0 {
        return Ok(());
    }
    counter.fetch_sub(1, Ordering::SeqCst);
    Err(anyhow::anyhow!(
        "__TEST_TRANSIENT__ injected transient FS failure"
    ))
}

fn parse_profile_kind(value: &str) -> Result<AuthProfileKind> {
    match value {
        "oauth" => Ok(AuthProfileKind::OAuth),
        "token" => Ok(AuthProfileKind::Token),
        other => anyhow::bail!("Unsupported auth profile kind: {other}"),
    }
}

fn profile_kind_to_string(kind: AuthProfileKind) -> &'static str {
    match kind {
        AuthProfileKind::OAuth => "oauth",
        AuthProfileKind::Token => "token",
    }
}

fn parse_optional_datetime(value: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    value.map(parse_datetime).transpose()
}

fn parse_datetime(value: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|dt| dt.with_timezone(&Utc))
        .with_context(|| format!("Invalid RFC3339 timestamp: {value}"))
}

fn parse_datetime_with_fallback(value: &str) -> DateTime<Utc> {
    parse_datetime(value).unwrap_or_else(|_| Utc::now())
}

pub fn profile_id(provider: &str, profile_name: &str) -> String {
    format!("{}:{}", provider.trim(), profile_name.trim())
}

pub fn normalize_profile_id_provider(id: &str) -> String {
    id.rsplit_once(':').map_or_else(
        || id.to_ascii_lowercase(),
        |(provider, name)| format!("{}:{name}", provider.to_ascii_lowercase()),
    )
}

fn quarantine_corrupt_store(path: &Path) -> Result<PathBuf> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("auth-profiles");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("json");
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let mut candidate = parent.join(format!("{stem}.corrupt-{ts}.{ext}"));
    let mut suffix = 0u32;
    while candidate.exists() {
        suffix += 1;
        candidate = parent.join(format!("{stem}.corrupt-{ts}-{suffix}.{ext}"));
    }
    fs::rename(path, &candidate).with_context(|| {
        format!(
            "Failed to quarantine corrupt auth profile store {} -> {}",
            path.display(),
            candidate.display()
        )
    })?;
    Ok(candidate)
}

#[cfg(test)]
#[path = "profiles_tests.rs"]
mod tests;
