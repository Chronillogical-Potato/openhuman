use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use rand::RngCore as _;
use sha2::{Digest, Sha256};

pub struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

impl Pkce {
    pub fn generate() -> Self {
        let mut bytes = [0u8; 64];
        rand::rng().fill_bytes(&mut bytes);
        let verifier = URL_SAFE_NO_PAD.encode(bytes);

        let hash = Sha256::digest(verifier.as_bytes());
        let challenge = URL_SAFE_NO_PAD.encode(hash);

        Pkce {
            verifier,
            challenge,
        }
    }
}

#[cfg(test)]
#[path = "pkce_tests.rs"]
mod tests;
