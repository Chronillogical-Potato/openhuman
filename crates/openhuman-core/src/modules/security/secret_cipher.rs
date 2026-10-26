//! Audited host orchestration for TinySecurity's routine config cipher bus.

use zeroize::{Zeroize, Zeroizing};

use super::{Client, SecurityError, cipher_consumer::Consumer};
use crate::security::approval::native::SecretScope;
use tinysecurity_bus::{
    EvaluateSecretCipherRequest2, ExecuteSecretCipherRequest2, SecretCipherKind2,
    SecretCipherRequest2, SecretCipherResponse2,
};

const OPERATION_TTL_MS: u32 = 30_000;

/// Successful result of the native routine config cipher.
pub(crate) enum Output {
    Ciphertext(String),
    Plaintext {
        value: Zeroizing<String>,
        migrated: bool,
        replacement_ciphertext: Option<String>,
    },
}

struct WipingRequest(SecretCipherRequest2);
impl Drop for WipingRequest {
    fn drop(&mut self) {
        self.0.input.zeroize();
    }
}

struct WipingEvaluate(EvaluateSecretCipherRequest2);
impl Drop for WipingEvaluate {
    fn drop(&mut self) {
        self.0.request.input.zeroize();
    }
}

struct WipingExecute(ExecuteSecretCipherRequest2);
impl Drop for WipingExecute {
    fn drop(&mut self) {
        self.0.request.input.zeroize();
    }
}

/// Perform the intent and execution phases on the registered subject's ordered
/// audit stream. Decrypted output is released only by the exact module-bound
/// consumer after the matching successful `ExecuteSecretCipher2` response.
pub(crate) async fn run(
    client: &Client,
    scope: &SecretScope,
    consumer: &Consumer,
    registration_id: &str,
    master_handle: &str,
    operation: SecretCipherKind2,
    input: &[u8],
) -> Result<Output, SecurityError> {
    let info = client.policy_info().await?;
    let _stream = scope.lock().await;
    let mut correlations = scope
        .correlations(info.generation, 4)
        .await
        .map_err(SecurityError::Unavailable)?
        .into_iter();
    let mut request = WipingRequest(SecretCipherRequest2 {
        registration_id: registration_id.to_owned(),
        master_handle: master_handle.to_owned(),
        call_id: uuid::Uuid::new_v4().simple().to_string(),
        operation,
        input: input.to_vec(),
        ttl_ms: OPERATION_TTL_MS,
    });
    let mut evaluate = WipingEvaluate(EvaluateSecretCipherRequest2 {
        request: request.0.clone(),
        audit: correlations
            .next()
            .ok_or_else(|| SecurityError::Unavailable("secret audit unavailable".into()))?,
    });
    let pending = client.evaluate_secret_cipher2(&mut evaluate.0).await?;
    let execute_request = std::mem::replace(
        &mut request.0,
        SecretCipherRequest2 {
            registration_id: String::new(),
            master_handle: String::new(),
            call_id: String::new(),
            operation,
            input: Vec::new(),
            ttl_ms: 0,
        },
    );
    let mut execute = WipingExecute(ExecuteSecretCipherRequest2 {
        request: execute_request,
        pending_id: pending.pending_id,
        audit: correlations
            .next()
            .ok_or_else(|| SecurityError::Unavailable("secret audit unavailable".into()))?,
        delivery_audit: correlations
            .next()
            .ok_or_else(|| SecurityError::Unavailable("secret audit unavailable".into()))?,
        completion_audit: correlations
            .next()
            .ok_or_else(|| SecurityError::Unavailable("secret audit unavailable".into()))?,
    });
    match client.execute_secret_cipher2(&mut execute.0).await? {
        SecretCipherResponse2::Ciphertext { ciphertext } => Ok(Output::Ciphertext(ciphertext)),
        SecretCipherResponse2::Delivered {
            identity,
            migrated,
            replacement_ciphertext,
        } => {
            let (plaintext, _replacement) = consumer
                .take(&identity)
                .map_err(|_| SecurityError::Unavailable("secret cipher output unavailable".into()))?;
            Ok(Output::Plaintext {
                value: plaintext,
                migrated,
                replacement_ciphertext,
            })
        }
    }
}

#[cfg(test)]
#[path = "secret_cipher_tests.rs"]
mod tests;
