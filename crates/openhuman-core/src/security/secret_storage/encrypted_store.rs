// Encrypted secret store — defense-in-depth for API keys and tokens.
//
// Secrets are encrypted using ChaCha20-Poly1305 AEAD with a random key stored
// in the OS keychain for normal app environments. Legacy installs may still
// have `{data_dir}/openhuman/.secret_key`; those keys are migrated into the
// keychain on first use. The config file stores only hex-encoded ciphertext, never
// plaintext keys.
//
// Each encryption generates a fresh random 12-byte nonce, prepended to the
// ciphertext. The Poly1305 authentication tag prevents tampering.
//
// This prevents:
//   - Plaintext exposure in config files
//   - Casual `grep` or `git log` leaks
//   - Accidental commit of raw API keys
//   - Known-plaintext attacks (unlike the previous XOR cipher)
//   - Ciphertext tampering (authenticated encryption)
//
// For sovereign users who prefer plaintext, `secrets.encrypt = false` disables this.
//
// Migration: values with the legacy `enc:` prefix (XOR cipher) are decrypted
// using the old algorithm for backward compatibility, but the config loader
// force-migrates them to `enc2:` on read (`decrypt_and_migrate`) and persists
// the upgrade so the insecure ciphertext never lingers on disk (audit C8). The
// legacy decode path logs a loud security warning every time it runs. New
// encryptions always produce `enc2:` (ChaCha20-Poly1305).

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

use tinystoragedrivers::secrets::crypto;

/// Length of the random encryption key in bytes (256-bit, matches `ChaCha20`).
const KEY_LEN: usize = crypto::KEY_LEN;

/// Keychain slot used for the SecretStore master key.
const KEYCHAIN_MASTER_KEY: &str = "secretstore.master_key";

/// Manages encrypted storage of secrets (API keys, tokens, etc.)
#[derive(Debug, Clone)]
pub struct SecretStore {
    /// Legacy path to the on-disk key file (`{data_dir}/openhuman/.secret_key`).
    /// Used for one-time migration into the keychain.
    key_path: PathBuf,
    /// Whether encryption is enabled
    enabled: bool,
    /// The exact host configuration used to admit the callback-bound module.
    config: Option<crate::config::Config>,
}

impl SecretStore {
    /// Create a new secret store rooted at the given directory.
    pub fn new(openhuman_dir: &Path, enabled: bool) -> Self {
        Self {
            key_path: openhuman_dir.join(".secret_key"),
            enabled,
            config: None,
        }
    }

    /// Create a store bound to the configuration whose callback authority it uses.
    pub fn new_with_config(
        openhuman_dir: &Path,
        enabled: bool,
        config: &crate::config::Config,
    ) -> Self {
        Self {
            key_path: openhuman_dir.join(".secret_key"),
            enabled,
            config: Some(config.clone()),
        }
    }

    /// Encrypt through the admitted TinySecurity cipher bus.
    pub async fn encrypt_via_security(&self, plaintext: &str) -> Result<String> {
        if !self.enabled || plaintext.is_empty() {
            return Ok(plaintext.to_owned());
        }
        match self
            .run_security_cipher(
                tinysecurity_bus::SecretCipherKind2::Encrypt,
                plaintext.as_bytes(),
            )
            .await?
        {
            crate::modules::security::SecretCipherOutput::Ciphertext(value) => Ok(value),
            crate::modules::security::SecretCipherOutput::Plaintext { .. } => {
                anyhow::bail!("secret encryption was not completed")
            }
        }
    }

    /// Decrypt through TinySecurity; unprefixed values retain plaintext compatibility.
    pub async fn decrypt_via_security(&self, value: &str) -> Result<String> {
        if !self.enabled || !Self::is_encrypted(value) {
            return Ok(value.to_owned());
        }
        match self
            .run_security_cipher(
                tinysecurity_bus::SecretCipherKind2::Decrypt,
                value.as_bytes(),
            )
            .await?
        {
            crate::modules::security::SecretCipherOutput::Plaintext { value, .. } => {
                Ok(value.to_string())
            }
            crate::modules::security::SecretCipherOutput::Ciphertext(_) => {
                anyhow::bail!("secret decryption was not completed")
            }
        }
    }

    /// Decrypt and migrate legacy ciphertext through TinySecurity.
    pub async fn decrypt_and_migrate_via_security(
        &self,
        value: &str,
    ) -> Result<(String, Option<String>)> {
        if !self.enabled || !Self::is_encrypted(value) {
            return Ok((value.to_owned(), None));
        }
        match self
            .run_security_cipher(
                tinysecurity_bus::SecretCipherKind2::DecryptAndMigrate,
                value.as_bytes(),
            )
            .await?
        {
            crate::modules::security::SecretCipherOutput::Plaintext {
                value,
                migrated,
                replacement_ciphertext,
            } => Ok((
                value.to_string(),
                migrated.then_some(replacement_ciphertext).flatten(),
            )),
            crate::modules::security::SecretCipherOutput::Ciphertext(_) => {
                anyhow::bail!("secret migration was not completed")
            }
        }
    }

    async fn run_security_cipher(
        &self,
        operation: tinysecurity_bus::SecretCipherKind2,
        input: &[u8],
    ) -> Result<crate::modules::security::SecretCipherOutput> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("TinySecurity configuration is unavailable"))?;
        #[cfg(feature = "security-module")]
        {
            let scope = crate::modules::ops::security_secret_scope()
                .await
                .ok_or_else(|| anyhow::anyhow!("TinySecurity secret scope is unavailable"))?;
            let consumer = crate::modules::ops::security_cipher_consumer()
                .await
                .ok_or_else(|| anyhow::anyhow!("TinySecurity cipher consumer is unavailable"))?;
            let registration_id = crate::modules::ops::security_secret_registration_id()
                .await
                .ok_or_else(|| {
                    anyhow::anyhow!("TinySecurity secret registration is unavailable")
                })?;
            let client = crate::modules::security::client(config)
                .await
                .map_err(anyhow::Error::new)?;
            let generation = client
                .policy_info()
                .await
                .map_err(anyhow::Error::new)?
                .generation;
            let cache_key = normalize_cache_path(&self.key_path);
            let master_handle = if let Some(handle) = cached_master_handle(&cache_key, generation) {
                handle
            } else {
                let handle = crate::modules::security::resolve_user_master(
                    &client,
                    &scope,
                    &registration_id,
                )
                .await
                .map_err(anyhow::Error::new)?;
                cache_master_handle(&cache_key, generation, &handle);
                handle
            };
            crate::modules::security::run_secret_cipher(
                &client,
                &scope,
                &consumer,
                &registration_id,
                &master_handle,
                operation,
                input,
            )
            .await
            .map_err(anyhow::Error::new)
        }
        #[cfg(not(feature = "security-module"))]
        {
            let _ = (config, operation, input);
            anyhow::bail!("TinySecurity is not built into this host")
        }
    }

    /// Encrypt a plaintext secret. Returns hex-encoded ciphertext prefixed with `enc2:`.
    /// Format: `enc2:<hex(nonce ‖ ciphertext ‖ tag)>` (12 + N + 16 bytes).
    /// If encryption is disabled, returns the plaintext as-is.
    pub fn encrypt(&self, plaintext: &str) -> Result<String> {
        if !self.enabled || plaintext.is_empty() {
            return Ok(plaintext.to_string());
        }

        let key = self.cipher_key()?;
        tinystoragedrivers::secrets::crypto::encrypt_enc2(&key, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))
    }

    /// Decrypt a secret.
    /// - `enc2:` prefix → ChaCha20-Poly1305 (current format)
    /// - `enc:` prefix → legacy XOR cipher (backward compatibility for migration)
    /// - No prefix → returned as-is (plaintext config)
    ///
    /// **Warning**: Legacy `enc:` values are insecure. Use `decrypt_and_migrate` to
    /// automatically upgrade them to the secure `enc2:` format.
    pub fn decrypt(&self, value: &str) -> Result<String> {
        if let Some(hex_str) = value.strip_prefix("enc2:") {
            self.decrypt_chacha20(hex_str)
        } else if let Some(hex_str) = value.strip_prefix("enc:") {
            self.decrypt_legacy_xor(hex_str)
        } else {
            Ok(value.to_string())
        }
    }

    /// Decrypt one existing `enc2:` value using an explicitly supplied key.
    /// This does not consult or initialize the process keyring and does not
    /// migrate legacy `enc:` values.
    pub(crate) fn decrypt_with_existing_key(value: &str, key: &str) -> Result<String> {
        let Some(_hex_str) = value.strip_prefix("enc2:") else {
            if value.starts_with("enc:") {
                anyhow::bail!("legacy encrypted value is unavailable to read-only inspection");
            }
            return Ok(value.to_string());
        };

        let key = crypto::key_from_hex(key.trim())
            .map_err(|_| anyhow::anyhow!("invalid secret key length"))?;
        let plaintext = crypto::decrypt_enc2(&key, value)
            .map_err(|_| anyhow::anyhow!("decryption failed — wrong key or tampered data"))?;
        String::from_utf8(plaintext.to_vec()).context("decrypted secret is not valid UTF-8")
    }

    /// Decrypt a secret and return a migrated `enc2:` value if the input used legacy `enc:` format.
    ///
    /// Returns `(plaintext, Some(new_enc2_value))` if migration occurred, or
    /// `(plaintext, None)` if no migration was needed.
    ///
    /// This allows callers to persist the upgraded value back to config.
    pub fn decrypt_and_migrate(&self, value: &str) -> Result<(String, Option<String>)> {
        if let Some(hex_str) = value.strip_prefix("enc2:") {
            // Already using secure format — no migration needed
            let plaintext = self.decrypt_chacha20(hex_str)?;
            Ok((plaintext, None))
        } else if let Some(hex_str) = value.strip_prefix("enc:") {
            // Legacy XOR cipher — decrypt and re-encrypt with ChaCha20-Poly1305
            log::warn!(
                "Decrypting legacy XOR-encrypted secret (enc: prefix). \
                 This format is insecure and will be removed in a future release. \
                 The secret will be automatically migrated to enc2: (ChaCha20-Poly1305)."
            );
            let plaintext = self.decrypt_legacy_xor(hex_str)?;
            let migrated = self.encrypt(&plaintext)?;
            Ok((plaintext, Some(migrated)))
        } else {
            // Plaintext — no migration needed
            Ok((value.to_string(), None))
        }
    }

    /// Check if a value uses the legacy `enc:` format that should be migrated.
    pub fn needs_migration(value: &str) -> bool {
        value.starts_with("enc:")
    }

    /// Decrypt using ChaCha20-Poly1305 (current secure format).
    fn decrypt_chacha20(&self, hex_str: &str) -> Result<String> {
        let key = self.cipher_key()?;
        let ciphertext = format!("enc2:{hex_str}");
        let plaintext_bytes = crypto::decrypt_enc2(&key, &ciphertext)
            .map_err(|_| anyhow::anyhow!("Decryption failed — wrong key or tampered data"))?;

        String::from_utf8(plaintext_bytes.to_vec())
            .context("Decrypted secret is not valid UTF-8 — corrupt data")
    }

    /// The config encryption key as the fixed-size array the cipher takes.
    fn cipher_key(&self) -> Result<Zeroizing<[u8; KEY_LEN]>> {
        let bytes = self.load_or_create_key()?;
        let key: [u8; KEY_LEN] = bytes.as_slice().try_into().map_err(|_| {
            anyhow::anyhow!(
                "config encryption key is {} bytes, expected {KEY_LEN}",
                bytes.len()
            )
        })?;
        Ok(Zeroizing::new(key))
    }

    /// Decrypt using legacy XOR cipher (insecure, for backward compatibility only).
    ///
    /// This is the single choke-point for the insecure `enc:` format, so it logs
    /// a loud security warning every time it runs. The repeating-key XOR is
    /// unauthenticated and leaks the master key to anyone who knows (or can guess
    /// the structure of) the plaintext — e.g. `xoxb-`/`sk-` token prefixes give
    /// `key[i] = ct[i] XOR pt[i]`. Callers on the read path should prefer
    /// [`decrypt_and_migrate`](Self::decrypt_and_migrate) so the value is rewritten
    /// to `enc2:` and the legacy ciphertext stops persisting on disk.
    fn decrypt_legacy_xor(&self, hex_str: &str) -> Result<String> {
        log::warn!(
            "[security] decrypting legacy XOR-encrypted secret (enc: prefix). \
             This format is INSECURE (unauthenticated repeating-key XOR; known-plaintext \
             recovers the master key) and must be migrated to enc2: (ChaCha20-Poly1305). \
             It will be removed in a future release — re-save your config or settings to \
             force migration."
        );
        let ciphertext = hex_decode(hex_str)
            .context("Failed to decode legacy encrypted secret (corrupt hex)")?;
        let key = self.load_or_create_key()?;
        let plaintext_bytes = xor_cipher(&ciphertext, &key);
        String::from_utf8(plaintext_bytes)
            .context("Decrypted legacy secret is not valid UTF-8 — wrong key or corrupt data")
    }

    /// Check if a value is already encrypted (current or legacy format).
    pub fn is_encrypted(value: &str) -> bool {
        value.starts_with("enc2:") || value.starts_with("enc:")
    }

    fn keyring_user_id(&self) -> String {
        keyring_user_id_from_dir(self.key_path.parent().unwrap_or_else(|| Path::new(".")))
    }

    fn load_or_create_key_from_keyring(&self) -> Result<Zeroizing<Vec<u8>>> {
        let user_id = self.keyring_user_id();
        if !crate::security::secret_storage::store::backend().is_persistent() {
            // A volatile provider must not absorb and delete a legacy disk key.
            // Read existing keys without migration; new keys stay in memory.
            if self.key_path.exists() {
                return decode_key_hex(fs::read_to_string(&self.key_path)?.trim());
            }
            let key = crate::security::secret_storage::get_or_create_random(
                &user_id,
                KEYCHAIN_MASTER_KEY,
                KEY_LEN,
            )
            .map_err(anyhow::Error::new)?;
            return decode_key_hex(key.trim());
        }
        match crate::security::secret_storage::migrate_from_file(
            &user_id,
            KEYCHAIN_MASTER_KEY,
            &self.key_path,
        ) {
            Ok(crate::security::secret_storage::MigrationOutcome::MigratedAndDeleted) => {
                log::info!(
                    "[security] migrated legacy secret-store key into keychain from {}",
                    self.key_path.display()
                );
            }
            Ok(crate::security::secret_storage::MigrationOutcome::AlreadyMigrated)
            | Ok(crate::security::secret_storage::MigrationOutcome::NoSourceFile) => {}
            Err(error) => {
                log::warn!(
                    "[security] failed to migrate legacy secret-store key from {}: {}",
                    self.key_path.display(),
                    error.diagnostic()
                );
            }
        }

        let hex_key = crate::security::secret_storage::get_or_create_random(
            &user_id,
            KEYCHAIN_MASTER_KEY,
            KEY_LEN,
        )
        .map_err(|e| {
            anyhow::anyhow!(
                "Failed to load secret-store master key from keychain: {}",
                e.diagnostic()
            )
        })?;
        decode_key_hex(hex_key.trim())
    }

    /// Load the encryption key from keychain-backed storage.
    ///
    /// The decoded key is cached process-wide keyed by `key_path`, so repeated
    /// callers hit memory instead of the keychain lookup.
    fn load_or_create_key(&self) -> Result<Zeroizing<Vec<u8>>> {
        let cache_key_path = normalize_cache_path(&self.key_path);
        if let Some(cached) = cached_key(&cache_key_path) {
            return Ok(cached);
        }

        let key = self.load_or_create_key_from_keyring()?;
        cache_key(&cache_key_path, &key);
        Ok(key)
    }
}

struct CachedMasterHandle {
    generation: u64,
    expires: Instant,
    handle: String,
}

const MASTER_HANDLE_CACHE_TTL: Duration = Duration::from_secs(20);

fn master_handle_cache() -> &'static Mutex<HashMap<PathBuf, CachedMasterHandle>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, CachedMasterHandle>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_master_handle(path: &Path, generation: u64) -> Option<String> {
    let mut cache = master_handle_cache().lock().ok()?;
    cache.retain(|_, value| value.expires > Instant::now());
    cache
        .get(path)
        .and_then(|value| (value.generation == generation).then(|| value.handle.clone()))
}

fn cache_master_handle(path: &Path, generation: u64, handle: &str) {
    if let Ok(mut cache) = master_handle_cache().lock() {
        cache.insert(
            path.to_path_buf(),
            CachedMasterHandle {
                generation,
                expires: Instant::now() + MASTER_HANDLE_CACHE_TTL,
                handle: handle.to_owned(),
            },
        );
    }
}

pub(crate) fn keyring_user_id_from_dir(openhuman_dir: &Path) -> String {
    if let Some(id) = openhuman_dir.file_name().and_then(|s| s.to_str()) {
        if !id.is_empty() {
            return id.to_string();
        }
    }

    let path_str = openhuman_dir.to_string_lossy();
    let mut hash: u64 = 14695981039346656037u64;
    for b in path_str.as_bytes() {
        hash ^= u64::from(*b);
        hash = hash.wrapping_mul(1099511628211u64);
    }
    format!("secretstore-path-{hash:016x}")
}

/// Normalize a path into a stable cache key. Tries `canonicalize` first (so
/// symlinks, relative paths, and Windows case-variants all collapse to the
/// same key), falls back to `std::path::absolute` when the file does not yet
/// exist (e.g. the create branch in `load_or_create_key`), and finally to the
/// raw path so a normalization failure never breaks the cache.
fn normalize_cache_path(path: &Path) -> PathBuf {
    fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf())
}

/// Process-wide cache of decoded key bytes keyed by absolute path.
///
/// Loading the key once per process is both faster and more reliable than
/// re-reading `.secret_key` on every decrypt. On Windows the file can be
/// transiently inaccessible (AV scanners holding a handle), and re-reading
/// turned that transient failure into a perma-failure for every subsequent
/// RPC call.
fn key_cache() -> &'static Mutex<HashMap<PathBuf, Zeroizing<Vec<u8>>>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Zeroizing<Vec<u8>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached_key(path: &Path) -> Option<Zeroizing<Vec<u8>>> {
    key_cache().lock().ok()?.get(path).cloned()
}

fn cache_key(path: &Path, key: &[u8]) {
    if let Ok(mut cache) = key_cache().lock() {
        // Stored as `Zeroizing` so the cached copy is wiped from memory when the
        // entry is removed (e.g. via `clear_cached_key`) or the process exits.
        cache.insert(path.to_path_buf(), Zeroizing::new(key.to_vec()));
    }
}

/// Read the key file, retrying transient errors a handful of times.
///
/// Windows AV scanners (Defender, etc.) routinely hold short-lived read
/// handles right after a file is created, which surfaces as
/// `ERROR_SHARING_VIOLATION` (raw OS error 32) or `PermissionDenied`. A few
/// short backoffs are enough to ride over the lock; the typical successful
/// path returns on the first attempt with zero added latency.
fn read_key_file_with_retry(path: &Path) -> std::io::Result<String> {
    use std::io::ErrorKind;

    const MAX_ATTEMPTS: u32 = 5;
    let mut last_err: Option<std::io::Error> = None;
    for attempt in 0..MAX_ATTEMPTS {
        match fs::read_to_string(path) {
            Ok(contents) => return Ok(contents),
            Err(err) => {
                let transient = matches!(
                    err.kind(),
                    ErrorKind::PermissionDenied | ErrorKind::Interrupted | ErrorKind::WouldBlock
                ) || err.raw_os_error() == Some(32); // ERROR_SHARING_VIOLATION (Windows)
                last_err = Some(err);
                if !transient || attempt + 1 == MAX_ATTEMPTS {
                    break;
                }
                let backoff_ms = 10u64 << attempt; // 10, 20, 40, 80 ms
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
        }
    }
    Err(last_err.unwrap_or_else(|| std::io::Error::other("read_to_string failed")))
}

/// XOR cipher with repeating key. Same function for encrypt and decrypt.
fn xor_cipher(data: &[u8], key: &[u8]) -> Vec<u8> {
    if key.is_empty() {
        return data.to_vec();
    }
    data.iter()
        .enumerate()
        .map(|(i, &b)| b ^ key[i % key.len()])
        .collect()
}

fn decode_key_hex(hex_key: &str) -> Result<Zeroizing<Vec<u8>>> {
    let key = crypto::key_from_hex(hex_key).context("Secret key file is corrupt")?;
    Ok(Zeroizing::new(key.to_vec()))
}

fn hex_decode(hex: &str) -> Result<Vec<u8>> {
    ::hex::decode(hex).map_err(anyhow::Error::from)
}

#[cfg(test)]
#[path = "encrypted_store_tests.rs"]
mod tests;
