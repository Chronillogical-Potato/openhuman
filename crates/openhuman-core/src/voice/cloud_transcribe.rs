//! OpenHuman authentication adapter for hosted speech-to-text.

use crate::api::config::effective_backend_api_url;
use crate::api::jwt::get_session_token;
use crate::api::BackendOAuthClient;
use crate::config::Config;
use crate::rpc::RpcOutcome;

pub use tinyinference_voice::cloud::{CloudTranscribeOptions, CloudTranscribeResult};

/// Transcribe renderer-supplied base64 audio through the hosted backend.
pub async fn transcribe_cloud(
    config: &Config,
    audio_base64: &str,
    options: &CloudTranscribeOptions,
) -> Result<RpcOutcome<CloudTranscribeResult>, String> {
    let token = get_session_token(config)
        .map_err(|error| error.to_string())?
        .filter(|token| !token.trim().is_empty())
        .ok_or_else(|| "no backend session token; sign in first".to_string())?;
    let client = BackendOAuthClient::new(&effective_backend_api_url(&config.api_url))
        .map_err(|error| error.to_string())?;
    let url = client
        .url_for("/openai/v1/audio/transcriptions")
        .map_err(|error| error.to_string())?;
    let http = client
        .raw_client()
        .map_err(crate::api::flatten_authed_error)?;
    let result =
        tinyinference_voice::cloud::transcribe(&http, url, &token, audio_base64, options).await?;
    Ok(RpcOutcome::single_log(
        result,
        "cloud STT via POST /openai/v1/audio/transcriptions",
    ))
}
