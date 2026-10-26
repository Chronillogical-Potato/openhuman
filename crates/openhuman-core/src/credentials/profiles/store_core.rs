//! Construction and the public CRUD surface of [`AuthProfilesStore`].

use anyhow::Result;
use chrono::Utc;
use std::path::Path;

use super::{AuthProfile, AuthProfilesData, AuthProfilesStore};
use crate::security::secret_storage::SecretStore;

use super::LOCK_FILENAME;

impl AuthProfilesStore {
    pub fn new(state_dir: &Path, encrypt_secrets: bool) -> Self {
        Self::construct(state_dir, encrypt_secrets, None)
    }

    pub fn new_with_config(
        state_dir: &Path,
        encrypt_secrets: bool,
        config: &crate::config::Config,
    ) -> Self {
        #[cfg(feature = "security-module")]
        crate::modules::security::sync_seam::arm_current();
        Self::construct(state_dir, encrypt_secrets, Some(config.clone()))
    }

    fn construct(
        state_dir: &Path,
        encrypt_secrets: bool,
        security_config: Option<crate::config::Config>,
    ) -> Self {
        let user_id = super::user_id_from_state_dir(state_dir);
        let use_keychain = security_config.is_some() && cfg!(feature = "security-module");
        log::debug!(
            "[auth] AuthProfilesStore::new state_dir={} user_id={user_id} use_keychain={use_keychain}",
            state_dir.display()
        );
        Self {
            path: super::profiles_file(state_dir),
            legacy_path: super::legacy_profiles_file(state_dir),
            lock_path: state_dir.join(LOCK_FILENAME),
            secret_store: SecretStore::new(state_dir, encrypt_secrets),
            user_id,
            use_keychain,
            security_config,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Result<AuthProfilesData> {
        match self.acquire_read_lock()? {
            Some(_lock) => self.load_locked(),
            None => {
                log::warn!(
                    "[auth] profile lease unavailable on a full or read-only filesystem; \
                     serving a lock-free read-only snapshot"
                );
                self.load_unlocked_readonly()
            }
        }
    }

    pub fn upsert_profile(&self, mut profile: AuthProfile, set_active: bool) -> Result<()> {
        let _lock = self.acquire_lock()?;
        let mut data = self.load_locked()?;

        profile.updated_at = Utc::now();
        if let Some(existing) = data.profiles.get(&profile.id) {
            profile.created_at = existing.created_at;
        }

        if set_active {
            data.active_profiles
                .insert(profile.provider.clone(), profile.id.clone());
        }

        data.profiles.insert(profile.id.clone(), profile);
        data.updated_at = Utc::now();

        self.save_locked(&data)
    }

    pub fn remove_profile(&self, profile_id: &str) -> Result<bool> {
        let _lock = self.acquire_lock()?;
        let persisted_before = self.read_persisted_locked()?;
        let mut data = self.load_locked()?;

        let removed = data.profiles.remove(profile_id).is_some();
        if !removed {
            return Ok(false);
        }

        data.active_profiles
            .retain(|_, active| active != profile_id);
        data.updated_at = Utc::now();
        self.save_locked(&data)?;

        // Remove the TinySecurity secret after persisting metadata removal.
        // If the user denies or the module is unavailable, restore the exact
        // prior document so an orphaned/stranded keyring entry stays reachable.
        if self.use_keychain {
            if let Err(delete_error) = self.keychain_delete_secrets(profile_id) {
                return self.rollback_profile_removal(&persisted_before, delete_error);
            }
        }

        Ok(true)
    }

    pub(super) fn rollback_profile_removal(
        &self,
        persisted_before: &super::PersistedAuthProfiles,
        delete_error: anyhow::Error,
    ) -> Result<()> {
        if let Err(rollback_error) = self.write_persisted_locked(persisted_before) {
            anyhow::bail!(
                "keyring delete failed and profile metadata rollback failed: {delete_error}; {rollback_error}"
            );
        }
        Err(delete_error.context("profile removal was not completed"))
    }

    pub fn set_active_profile(&self, provider: &str, profile_id: &str) -> Result<()> {
        let _lock = self.acquire_lock()?;
        let mut data = self.load_locked()?;

        if !data.profiles.contains_key(profile_id) {
            anyhow::bail!("Auth profile not found: {profile_id}");
        }

        data.active_profiles
            .insert(provider.to_ascii_lowercase(), profile_id.to_string());
        data.updated_at = Utc::now();
        self.save_locked(&data)
    }

    pub fn clear_active_profile(&self, provider: &str) -> Result<()> {
        let _lock = self.acquire_lock()?;
        let mut data = self.load_locked()?;
        data.active_profiles.remove(&provider.to_ascii_lowercase());
        data.updated_at = Utc::now();
        self.save_locked(&data)
    }

    pub fn update_profile<F>(&self, profile_id: &str, mut updater: F) -> Result<AuthProfile>
    where
        F: FnMut(&mut AuthProfile) -> Result<()>,
    {
        let _lock = self.acquire_lock()?;
        let mut data = self.load_locked()?;

        let profile = data
            .profiles
            .get_mut(profile_id)
            .ok_or_else(|| anyhow::anyhow!("Auth profile not found: {profile_id}"))?;

        updater(profile)?;
        profile.updated_at = Utc::now();
        let updated_profile = profile.clone();
        data.updated_at = Utc::now();
        self.save_locked(&data)?;
        Ok(updated_profile)
    }

    pub(super) fn load_locked(&self) -> Result<AuthProfilesData> {
        self.load_resolved(true)
    }

    /// Lock-free read-only load used as the [`AuthProfilesStore::load`]
    /// fallback when the exclusive lock can't be created because the
    /// filesystem won't accept the lock file (disk full / read-only mount —
    /// Sentry TAURI-RUST-4SZ). Safe without the lock because writers publish
    /// the store atomically (tmp + `fs::rename`), so a bare read always sees
    /// a complete file. Skips the opportunistic migration / dropped-profile
    /// rewrite that `load_locked` performs — that write needs both the lock
    /// and a writable disk, and this path runs precisely when neither holds.
    pub(super) fn load_unlocked_readonly(&self) -> Result<AuthProfilesData> {
        self.load_resolved(false)
    }
}
