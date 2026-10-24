//! Host adapter for TinySecurity device identities and tunnel ciphers.
//!
//! Cryptographic framing, X25519, HKDF, AEAD and replay protection are owned
//! by `tinysecurity-crypto`; this module only keeps the host's encrypted-key
//! persistence format and base64url encoding.

use zeroize::Zeroizing;

/// A persisted core identity backed by TinySecurity's native X25519 key owner.
pub struct DeviceKeypair {
    private: Zeroizing<[u8; 32]>,
    native: tinysecurity_crypto::device::KeyPair,
    /// Base64url-encoded public key returned in the QR payload.
    pub pubkey_b64: String,
}

impl DeviceKeypair {
    /// Generate a core identity using operating-system entropy.
    pub fn generate() -> Self {
        let bytes = Zeroizing::new(rand::random::<[u8; 32]>());
        Self::from_private_bytes(bytes).expect("generated X25519 key is valid")
    }

    /// Reconstruct an identity from secret bytes obtained from authenticated
    /// encrypted storage.
    pub fn from_private_bytes(bytes: Zeroizing<[u8; 32]>) -> Result<Self, String> {
        if *bytes == [0; 32] {
            return Err("[devices/crypto] invalid zero private key".into());
        }
        let native = tinysecurity_crypto::device::KeyPair::from_secret(bytes.clone())
            .map_err(|error| format!("[devices/crypto] invalid private key: {error}"))?;
        let pubkey_b64 = native.public_key().encode();
        Ok(Self {
            private: bytes,
            native,
            pubkey_b64,
        })
    }

    /// Serialize the identity for encryption by the host SecretStore.
    pub fn private_bytes(&self) -> [u8; 32] {
        *self.private
    }

    /// Open the existing sealed v3 handshake frame.
    pub fn open_handshake(
        &self,
        frame: &[u8],
    ) -> tinysecurity_crypto::Result<(tinysecurity_crypto::device::PublicKey, Zeroizing<Vec<u8>>)>
    {
        self.native.open_handshake(frame)
    }

    /// Derive server tunnel keys using TinySecurity's wire-compatible schedule.
    pub fn session_keys(
        &self,
        device_static: tinysecurity_crypto::device::PublicKey,
        server_ephemeral: &tinysecurity_crypto::device::KeyPair,
        client_ephemeral: tinysecurity_crypto::device::PublicKey,
        server_public: tinysecurity_crypto::device::PublicKey,
    ) -> tinysecurity_crypto::Result<tinysecurity_crypto::device::SessionKeys> {
        self.native.session_keys(
            device_static,
            server_ephemeral,
            client_ephemeral,
            client_ephemeral,
            server_public,
        )
    }

    /// Seal the existing v4 handshake acknowledgement.
    pub fn seal_ack(
        &self,
        client_ephemeral: tinysecurity_crypto::device::PublicKey,
        plaintext: &[u8],
    ) -> tinysecurity_crypto::Result<Vec<u8>> {
        self.native.seal_ack(client_ephemeral, plaintext)
    }
}

pub fn base64url_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

pub fn base64url_decode(s: &str) -> Result<Vec<u8>, String> {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(s)
        .map_err(|error| format!("base64url decode error: {error}"))
}
