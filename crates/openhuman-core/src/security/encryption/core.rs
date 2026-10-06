use aes_gcm::aead::rand_core::RngCore;
use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use argon2::{self, Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Salt length for Argon2id key derivation
const SALT_LENGTH: usize = 16;
/// Nonce length for AES-256-GCM (96 bits)
const NONCE_LENGTH: usize = 12;
/// Derived key length (256 bits for AES-256)
const KEY_LENGTH: usize = 32;

/// Encrypted payload with metadata for decryption
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EncryptedPayload {
    /// AES-256-GCM ciphertext
    pub ciphertext: Vec<u8>,
    /// Random nonce used for this encryption
    pub nonce: Vec<u8>,
    /// Argon2id salt used for key derivation
    pub salt: Vec<u8>,
}

/// Encryption key material
#[derive(Clone)]
pub struct EncryptionKey {
    key_bytes: [u8; KEY_LENGTH],
}

impl EncryptionKey {
    /// Derive an encryption key from a password and salt using Argon2id.
    pub fn derive(password: &str, salt: &[u8]) -> Result<Self, String> {
        let params = Params::new(65536, 3, 1, Some(KEY_LENGTH))
            .map_err(|e| format!("Argon2 params error: {e}"))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

        let mut key_bytes = [0u8; KEY_LENGTH];
        argon2
            .hash_password_into(password.as_bytes(), salt, &mut key_bytes)
            .map_err(|e| format!("Key derivation failed: {e}"))?;

        Ok(Self { key_bytes })
    }

    /// Generate a new random salt for key derivation.
    pub fn generate_salt() -> Vec<u8> {
        let mut salt = vec![0u8; SALT_LENGTH];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    /// Encrypt plaintext bytes.
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedPayload, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key_bytes).map_err(|e| format!("Cipher init: {e}"))?;

        let mut nonce_bytes = [0u8; NONCE_LENGTH];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| format!("Encryption failed: {e}"))?;

        Ok(EncryptedPayload {
            ciphertext,
            nonce: nonce_bytes.to_vec(),
            salt: Vec::new(), // Salt is stored separately in the key file
        })
    }

    /// Decrypt an encrypted payload.
    pub fn decrypt(&self, payload: &EncryptedPayload) -> Result<Vec<u8>, String> {
        let cipher =
            Aes256Gcm::new_from_slice(&self.key_bytes).map_err(|e| format!("Cipher init: {e}"))?;

        let nonce = Nonce::from_slice(&payload.nonce);

        cipher
            .decrypt(nonce, payload.ciphertext.as_ref())
            .map_err(|e| format!("Decryption failed: {e}"))
    }
}

/// Get the path to the OpenHuman data directory.
/// If an active user is set, returns the user-scoped directory under the
/// env-aware root returned by `default_root_openhuman_dir()`
/// (for example `~/.openhuman/users/{user_id}` in production or
/// `~/.openhuman-staging/users/{user_id}` when `OPENHUMAN_APP_ENV=staging`);
/// otherwise it falls back to that root directory itself.
pub fn get_data_dir() -> Result<PathBuf, String> {
    let root_dir = crate::config::default_root_openhuman_dir()
        .map_err(|e| format!("Cannot determine app data directory: {e}"))?;
    std::fs::create_dir_all(&root_dir)
        .map_err(|e| format!("Failed to create data directory: {e}"))?;

    let data_dir = if let Some(user_id) = crate::config::read_active_user_id(&root_dir) {
        let user_dir = crate::config::user_openhuman_dir(&root_dir, &user_id);
        std::fs::create_dir_all(&user_dir)
            .map_err(|e| format!("Failed to create user data directory: {e}"))?;
        user_dir
    } else {
        root_dir
    };

    Ok(data_dir)
}

#[cfg(test)]
#[path = "core_tests.rs"]
mod tests;
