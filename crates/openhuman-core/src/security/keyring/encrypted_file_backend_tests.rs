use super::*;
use std::cell::{Cell, RefCell};

/// In-memory fake of the keychain entry, so [`load_or_mint_master_key`] can
/// be exercised without a real OS keychain. `absent_error` is a fn pointer
/// because `keyring::Error` is not `Clone` — we mint a fresh error per call.
/// These tests touch no process-wide state (`load_or_mint_master_key` never
/// reads `MASTER_KEY`), so no OnceLock reset seam is needed.
struct FakeEntry {
    stored: RefCell<Option<String>>,
    absent_error: fn() -> keyring::Error,
    set_calls: Cell<usize>,
}

impl FakeEntry {
    fn with_stored(value: &str) -> Self {
        Self {
            stored: RefCell::new(Some(value.to_string())),
            absent_error: || keyring::Error::NoEntry,
            set_calls: Cell::new(0),
        }
    }
    fn absent(err: fn() -> keyring::Error) -> Self {
        Self {
            stored: RefCell::new(None),
            absent_error: err,
            set_calls: Cell::new(0),
        }
    }
}

impl MasterKeyEntry for FakeEntry {
    fn get_password(&self) -> Result<String, keyring::Error> {
        match &*self.stored.borrow() {
            Some(v) => Ok(v.clone()),
            None => Err((self.absent_error)()),
        }
    }
    fn set_password(&self, value: &str) -> Result<(), keyring::Error> {
        self.set_calls.set(self.set_calls.get() + 1);
        *self.stored.borrow_mut() = Some(value.to_string());
        Ok(())
    }
}

fn access_denied() -> keyring::Error {
    keyring::Error::NoStorageAccess(Box::new(std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        "keychain access denied",
    )))
}

fn platform_failure() -> keyring::Error {
    keyring::Error::PlatformFailure(Box::new(std::io::Error::other("platform boom")))
}

#[test]
fn loads_existing_key_without_minting() {
    let hex = "ab".repeat(KEY_LEN); // 32 bytes of 0xab
    let entry = FakeEntry::with_stored(&hex);
    let key = load_or_mint_master_key(&entry).expect("should load existing key");
    assert_eq!(key, [0xabu8; KEY_LEN]);
    assert_eq!(entry.set_calls.get(), 0, "must not overwrite existing key");
}

#[test]
fn mints_only_on_no_entry() {
    let entry = FakeEntry::absent(|| keyring::Error::NoEntry);
    let key = load_or_mint_master_key(&entry).expect("should mint when genuinely absent");
    assert_ne!(key, [0u8; KEY_LEN], "minted key should be random, not zero");
    assert_eq!(
        entry.set_calls.get(),
        1,
        "should store the freshly minted key"
    );
    // The key is now persisted, so a second load returns the same one.
    assert!(entry.stored.borrow().is_some());
}

#[test]
fn does_not_mint_on_access_denied() {
    // The #3311 case: existing key unreadable due to post-update ACL change.
    let entry = FakeEntry::absent(access_denied);
    let result = load_or_mint_master_key(&entry);
    assert!(result.is_err(), "access denial must NOT mint a new key");
    assert_eq!(
        entry.set_calls.get(),
        0,
        "must never call set_password on access denial — that orphans existing secrets"
    );
    assert!(
        entry.stored.borrow().is_none(),
        "keychain entry left untouched"
    );
}

#[test]
fn does_not_mint_on_platform_failure() {
    // Variant-independence: any non-NoEntry error fails safe, not just
    // NoStorageAccess (the exact macOS denial variant is unconfirmed).
    let entry = FakeEntry::absent(platform_failure);
    let result = load_or_mint_master_key(&entry);
    assert!(result.is_err(), "platform failure must NOT mint a new key");
    assert_eq!(entry.set_calls.get(), 0);
}

#[test]
fn rejects_wrong_length_key_without_minting() {
    let entry = FakeEntry::with_stored("abcd"); // 2 bytes, not KEY_LEN
    let result = load_or_mint_master_key(&entry);
    assert!(result.is_err(), "wrong-length stored key is an error");
    assert_eq!(
        entry.set_calls.get(),
        0,
        "must not overwrite on length mismatch"
    );
}

// ── Operator-supplied master key (#6926) ─────────────────────────────────────

fn hex_key(byte: u8) -> String {
    format!("{byte:02x}").repeat(KEY_LEN)
}

fn file_must_not_be_read(path: &Path) -> Result<String, String> {
    panic!("master key file {} must not be read", path.display())
}

#[test]
fn env_inline_key_is_used_and_names_its_source() {
    let hex = hex_key(0xab);
    let (key, source) = master_key_from_env(Some(&hex), None, file_must_not_be_read)
        .expect("valid inline key")
        .expect("inline key counts as supplied");
    assert_eq!(key, [0xabu8; KEY_LEN]);
    assert_eq!(source, MASTER_KEY_ENV);
}

#[test]
fn env_inline_key_tolerates_surrounding_whitespace_and_uppercase_hex() {
    let hex = format!("  {}\n", hex_key(0xcd).to_uppercase());
    let (key, _) = master_key_from_env(Some(&hex), None, file_must_not_be_read)
        .expect("trimmed key is valid")
        .expect("supplied");
    assert_eq!(key, [0xcdu8; KEY_LEN]);
}

#[test]
fn env_file_key_is_read_trimmed_and_names_the_variable() {
    let hex = hex_key(0x11);
    let (key, source) =
        master_key_from_env(None, Some("/run/secrets/openhuman_master_key"), |path| {
            assert_eq!(path, Path::new("/run/secrets/openhuman_master_key"));
            Ok(format!("{hex}\n"))
        })
        .expect("file key is valid")
        .expect("supplied");
    assert_eq!(key, [0x11u8; KEY_LEN]);
    assert_eq!(source, MASTER_KEY_FILE_ENV);
}

#[test]
fn unreadable_env_file_is_an_error_that_names_the_variable_not_the_path() {
    let err = master_key_from_env(None, Some("/nonexistent/master.key"), |_| {
        Err("cannot read master key file: boom".to_string())
    })
    .expect_err("unreadable file must not fall through to the keychain");
    assert!(err.contains(MASTER_KEY_FILE_ENV), "{err}");
    assert!(err.contains("cannot read"), "{err}");
    assert!(!err.contains("/nonexistent/master.key"), "{err}");
}

#[test]
fn a_key_pasted_into_the_file_variable_is_not_echoed() {
    // The operator meant `MASTER_KEY=<hex>` but set `MASTER_KEY_FILE=<hex>`:
    // the "path" is the secret, so the read error must not carry it into the
    // startup log.
    let pasted_key = hex_key(0x77);
    let err = master_key_from_env(None, Some(&pasted_key), read_master_key_file)
        .expect_err("a key is not a readable path");
    assert!(err.contains(MASTER_KEY_FILE_ENV), "{err}");
    assert!(
        !err.contains(&pasted_key),
        "error must not echo the key: {err}"
    );
}

#[test]
fn both_env_sources_set_is_rejected_before_reading_anything() {
    let hex = hex_key(0x22);
    let err = master_key_from_env(Some(&hex), Some("/run/secrets/key"), file_must_not_be_read)
        .expect_err("ambiguous configuration must be rejected");
    assert!(
        err.contains(MASTER_KEY_ENV) && err.contains(MASTER_KEY_FILE_ENV),
        "{err}"
    );
}

#[test]
fn malformed_env_key_is_rejected_without_leaking_the_value() {
    let too_short = "abcd";
    let err = master_key_from_env(Some(too_short), None, file_must_not_be_read)
        .expect_err("wrong length is rejected");
    assert!(err.contains("expected 64 hex characters, got 4"), "{err}");

    let not_hex = "zz".repeat(KEY_LEN);
    let err = master_key_from_env(Some(&not_hex), None, file_must_not_be_read)
        .expect_err("non-hex is rejected");
    assert!(err.contains("not valid hex"), "{err}");
    assert!(
        !err.contains(&not_hex),
        "error must not echo the value: {err}"
    );

    // Right character count, wrong bytes: must be rejected, not panic in the
    // byte-sliced hex decoder.
    let non_ascii = "é".repeat(KEY_LEN * 2);
    let err = master_key_from_env(Some(&non_ascii), None, file_must_not_be_read)
        .expect_err("non-ASCII is rejected");
    assert!(err.contains("not valid hex"), "{err}");

    // The same validation applies to a file's contents.
    let err = master_key_from_env(None, Some("/run/secrets/key"), |_| Ok("0123".to_string()))
        .expect_err("short file contents are rejected");
    assert!(
        err.contains(MASTER_KEY_FILE_ENV) && err.contains("got 4"),
        "{err}"
    );
}

#[test]
fn empty_env_values_fall_through_to_the_keychain() {
    assert!(master_key_from_env(None, None, file_must_not_be_read)
        .expect("nothing set is fine")
        .is_none());
    assert!(
        master_key_from_env(Some(""), Some("   "), file_must_not_be_read)
            .expect("empty values count as unset")
            .is_none()
    );
}

#[test]
fn read_master_key_file_returns_contents_and_reports_a_missing_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("master.key");
    std::fs::write(&path, format!("{}\n", hex_key(0x33))).unwrap();

    let contents = read_master_key_file(&path).expect("readable file");
    assert_eq!(contents.trim(), hex_key(0x33));

    let err = read_master_key_file(&tmp.path().join("missing.key")).expect_err("missing file");
    assert!(err.contains("cannot read master key file"), "{err}");
}

#[cfg(unix)]
#[test]
fn read_master_key_file_accepts_a_world_readable_secret_mount() {
    // Docker secrets are mounted 0444 and Kubernetes secret volumes 0644 by
    // default, read-only: permissive modes are warned about, never refused.
    use std::os::unix::fs::PermissionsExt;
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("master.key");
    std::fs::write(&path, hex_key(0x44)).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

    let contents = read_master_key_file(&path).expect("permissive mode is accepted");
    assert_eq!(contents, hex_key(0x44));
}

#[cfg(unix)]
#[test]
fn key_file_warning_covers_default_mounts_but_not_owner_or_fsgroup_modes() {
    // Warned: the Kubernetes and Docker defaults, and anything world-accessible.
    for mode in [0o644, 0o444, 0o604, 0o666, 0o602] {
        assert!(key_file_mode_is_world_accessible(mode), "{mode:04o}");
    }
    // Not warned: owner-only, and `defaultMode: 0400` under a pod `fsGroup`,
    // which kubelet turns into 0440.
    for mode in [0o400, 0o600, 0o440, 0o640] {
        assert!(!key_file_mode_is_world_accessible(mode), "{mode:04o}");
    }
}

#[test]
fn parse_master_key_hex_round_trips_an_encoded_key() {
    let key_bytes = crypto::generate_random_bytes(KEY_LEN);
    let parsed = parse_master_key_hex(&crypto::hex_encode(&key_bytes)).expect("valid");
    assert_eq!(parsed.as_slice(), key_bytes.as_slice());
}

#[test]
fn env_value_distinguishes_unset_from_invalid_unicode() {
    use std::env::VarError;

    assert_eq!(
        env_value(MASTER_KEY_ENV, Ok("abc".to_string())).expect("set"),
        Some("abc".to_string())
    );
    assert_eq!(
        env_value(MASTER_KEY_ENV, Err(VarError::NotPresent)).expect("unset is not an error"),
        None
    );

    // Invalid Unicode must be rejected, not treated as unset: falling through
    // to the keychain could mint a different key and orphan `secrets.enc`.
    let rejected = std::ffi::OsString::from("not-the-real-bytes");
    let err = env_value(MASTER_KEY_FILE_ENV, Err(VarError::NotUnicode(rejected)))
        .expect_err("invalid Unicode is a configuration error");
    assert!(err.contains(MASTER_KEY_FILE_ENV), "{err}");
    assert!(err.contains("not valid Unicode"), "{err}");
    assert!(
        !err.contains("not-the-real-bytes"),
        "error must not echo the value: {err}"
    );
}

// ── Wiring: the real env → `try_load_master_key` → key, no keychain ─────────
//
// `try_load_master_key` is uncached: it reads the environment on every call
// and never touches the process-wide `MASTER_KEY` (only `init_master_key`
// does, and no test here calls it; the one-time logic is tested through
// `init_once` with a local `OnceLock` below). Each test mutates the env only
// under `EnvVarGuard::locked()`, which serializes on the crate's shared
// `TEST_ENV_LOCK` and restores the previous values before releasing it, so
// the tests cannot observe each other's values in any order.

#[test]
fn try_load_master_key_prefers_the_inline_env_key_and_never_touches_the_keychain() {
    let hex = hex_key(0x55);
    let _env = crate::config::test_env::EnvVarGuard::locked()
        .with(MASTER_KEY_ENV, &hex)
        .without(MASTER_KEY_FILE_ENV);

    // A real OS keychain cannot be exercised under `cargo test` (the first
    // access blocks on a GUI prompt), so reaching the keychain here would hang
    // or fail; returning means the env source short-circuited it.
    let (key, source) = try_load_master_key().expect("inline env key loads");
    assert_eq!(key, [0x55u8; KEY_LEN]);
    assert_eq!(source, MASTER_KEY_ENV);
}

#[test]
fn try_load_master_key_reads_the_file_source_and_is_stable_across_restarts() {
    let tmp = tempfile::TempDir::new().unwrap();
    let path = tmp.path().join("master.key");
    std::fs::write(&path, format!("{}\n", hex_key(0x66))).unwrap();
    let _env = crate::config::test_env::EnvVarGuard::locked()
        .without(MASTER_KEY_ENV)
        .with(MASTER_KEY_FILE_ENV, &path);

    // Two loads stand in for two process starts: the same file must yield
    // the same key, so a secret encrypted before a restart decrypts after it.
    let (first, source) = try_load_master_key().expect("file key loads");
    let (second, _) = try_load_master_key().expect("file key loads again");
    assert_eq!(first, [0x66u8; KEY_LEN]);
    assert_eq!(first, second);
    assert_eq!(source, MASTER_KEY_FILE_ENV);

    let blob = crypto::chacha20_encrypt(&first, b"sk-live-secret").expect("encrypt");
    assert_eq!(
        crypto::chacha20_decrypt(&second, &blob).expect("decrypt with the restarted key"),
        b"sk-live-secret"
    );
}

#[test]
fn try_load_master_key_reports_a_configured_source_error_as_configured() {
    let _env = crate::config::test_env::EnvVarGuard::locked()
        .with(MASTER_KEY_ENV, "not-a-key")
        .without(MASTER_KEY_FILE_ENV);

    match try_load_master_key() {
        Err(MasterKeyError::Configured(e)) => {
            assert!(e.contains(MASTER_KEY_ENV), "{e}");
            assert!(!e.contains("not-a-key"), "must not echo the value: {e}");
        }
        other => panic!("expected a configuration error, got {other:?}"),
    }
}

#[test]
fn a_secret_written_under_a_configured_key_reads_back_after_a_restart() {
    let workspace = tempfile::TempDir::new().unwrap();
    let key_dir = tempfile::TempDir::new().unwrap();
    let key_path = key_dir.path().join("master.key");
    std::fs::write(&key_path, format!("{}\n", hex_key(0x88))).unwrap();
    let _env = crate::config::test_env::EnvVarGuard::locked()
        .without(MASTER_KEY_ENV)
        .with(MASTER_KEY_FILE_ENV, &key_path);

    // First "process": load the configured key and store a secret through the
    // same lock → read → encrypt → write path `KeyringBackend::set` uses.
    // `get`/`set` read the key from the process-wide `MASTER_KEY`, which
    // this test must not populate, so the key is passed explicitly.
    {
        let (key, _) = try_load_master_key().expect("configured key loads");
        let backend = EncryptedFileBackend::new(workspace.path());
        backend
            .set_with_key(&key, "provider:openai", "sk-live-secret")
            .expect("store under the configured key");
    }
    let on_disk = std::fs::read(workspace.path().join(SECRETS_FILENAME)).unwrap();
    assert!(
        !on_disk
            .windows(b"sk-live-secret".len())
            .any(|w| w == b"sk-live-secret"),
        "secrets.enc must not hold the plaintext"
    );

    // Second "process": a fresh key load and a fresh backend over the same
    // workspace read the secret back.
    let (key, _) = try_load_master_key().expect("configured key loads after restart");
    let backend = EncryptedFileBackend::new(workspace.path());
    assert_eq!(
        backend
            .get_with_key(&key, "provider:openai")
            .expect("read after restart")
            .as_deref(),
        Some("sk-live-secret")
    );
}

// ── One-time init keeps a configuration error ───────────────────────────────

#[test]
fn init_once_returns_the_configuration_error_on_every_call() {
    let cell = OnceLock::new();
    let first = init_once(&cell, || {
        Err(format!(
            "{MASTER_KEY_ENV}: expected 64 hex characters, got 4"
        ))
    });
    assert!(first.is_err(), "first call reports the error");

    // `OnceLock` does not rerun the closure; the cached error must still
    // surface rather than a silent `Ok` with no key loaded.
    let second = init_once(&cell, || panic!("init must run at most once"));
    assert_eq!(second, first);
}

#[test]
fn init_once_caches_a_loaded_key_and_a_keychain_outage_as_ok() {
    let loaded = OnceLock::new();
    assert_eq!(init_once(&loaded, || Ok(Some([0x99u8; KEY_LEN]))), Ok(()));
    assert_eq!(
        init_once(&loaded, || panic!("init must run at most once")),
        Ok(())
    );
    assert_eq!(loaded.get(), Some(&Ok(Some([0x99u8; KEY_LEN]))));

    // No key (non-encrypted backend, or the #3311 keychain outage) is not a
    // startup error.
    let unavailable = OnceLock::new();
    assert_eq!(init_once(&unavailable, || Ok(None)), Ok(()));
    assert_eq!(
        init_once(&unavailable, || panic!("init must run at most once")),
        Ok(())
    );
}
