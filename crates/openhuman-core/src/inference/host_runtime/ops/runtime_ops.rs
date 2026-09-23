//! Operations over the local runtime: status, summarize, prompt, vision,
//! embeddings, transcription, speech, and model asset management.

use chrono::Utc;

use crate::config::Config;
use crate::inference::host_runtime as local_ai;
use crate::inference::{
    LocalAiAssetsStatus, LocalAiDownloadsProgress, LocalAiEmbeddingResult, LocalAiSpeechResult,
    LocalAiStatus, LocalAiTtsResult,
};
use crate::rpc::RpcOutcome;

use super::turn_guards::enforce_user_prompt_or_reject;

/// Returns the current operational status of the local AI stack.
pub async fn local_ai_status(config: &Config) -> Result<RpcOutcome<LocalAiStatus>, String> {
    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let status = service.status();
    if matches!(status.state.as_str(), "idle" | "degraded") {
        let service_clone = service.clone();
        let config_clone = runtime.clone();
        tokio::spawn(async move {
            service_clone.bootstrap(&config_clone).await;
        });
    }
    // `LocalAiService` is a process-wide singleton whose cached `provider`
    // field was set at first init from whichever config it saw. After an
    // `inference_update_local_settings` call that swaps providers
    // (e.g. ollama → lm_studio) the cached value is stale, so we overlay
    // the current config's provider on the status snapshot before returning.
    let mut snapshot = service.status();
    snapshot.provider =
        tinyinference_local::provider::provider_from_name(&config.local_ai.provider)
            .as_str()
            .to_string();
    Ok(RpcOutcome::single_log(snapshot, "local ai status fetched"))
}

/// Generates a summary of the provided text using local AI models.
pub async fn local_ai_summarize(
    config: &Config,
    text: &str,
    max_tokens: Option<u32>,
) -> Result<RpcOutcome<String>, String> {
    enforce_user_prompt_or_reject(text.trim(), "local_ai.ops.local_ai_summarize")?;

    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let status = service.status();
    if !matches!(status.state.as_str(), "ready") {
        service.bootstrap(&runtime).await;
    }
    let summary = service
        .summarize_interactive(&runtime, text, max_tokens)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        summary,
        "local ai summarize completed",
    ))
}

/// Executes a raw prompt directly against the local AI model.
pub async fn local_ai_prompt(
    config: &Config,
    prompt: &str,
    max_tokens: Option<u32>,
    no_think: Option<bool>,
) -> Result<RpcOutcome<String>, String> {
    enforce_user_prompt_or_reject(prompt.trim(), "local_ai.ops.local_ai_prompt")?;

    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let status = service.status();
    if !matches!(status.state.as_str(), "ready") {
        service.bootstrap(&runtime).await;
    }
    let output = service
        .prompt_interactive(
            &runtime,
            prompt.trim(),
            max_tokens,
            no_think.unwrap_or(true),
        )
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(output, "local ai prompt completed"))
}

/// Executes a multimodal (vision) prompt with associated images.
pub async fn local_ai_vision_prompt(
    config: &Config,
    prompt: &str,
    image_refs: &[String],
    max_tokens: Option<u32>,
) -> Result<RpcOutcome<String>, String> {
    enforce_user_prompt_or_reject(prompt.trim(), "local_ai.ops.local_ai_vision_prompt")?;

    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let Some(_permit) = crate::cron::scheduler_gate::wait_for_capacity().await else {
        return Err("local AI vision inference is paused while signed out".to_string());
    };
    let output = service
        .vision_prompt(&runtime, prompt.trim(), image_refs, max_tokens)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai vision prompt completed",
    ))
}

/// Generates semantic embeddings for the provided input strings.
pub async fn local_ai_embed(
    config: &Config,
    inputs: &[String],
) -> Result<RpcOutcome<LocalAiEmbeddingResult>, String> {
    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let Some(_permit) = crate::cron::scheduler_gate::wait_for_capacity().await else {
        return Err("local AI embedding inference is paused while signed out".to_string());
    };
    let output = service
        .embed(&runtime, inputs)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai embedding completed",
    ))
}

/// Transcribes the audio file at the specified path.
pub async fn local_ai_transcribe(
    config: &Config,
    audio_path: &str,
) -> Result<RpcOutcome<LocalAiSpeechResult>, String> {
    let service = local_ai::global(config);
    let output = local_ai::service::transcribe(&service, config, audio_path.trim())
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai transcription completed",
    ))
}

/// Transcribes raw audio bytes by first saving them to a temporary file.
pub async fn local_ai_transcribe_bytes(
    config: &Config,
    audio_bytes: &[u8],
    extension: Option<String>,
) -> Result<RpcOutcome<LocalAiSpeechResult>, String> {
    let service = local_ai::global(config);

    let ext = extension
        .unwrap_or_else(|| "webm".to_string())
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if ext.is_empty() || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Invalid audio extension".to_string());
    }

    let voice_dir = std::env::temp_dir().join("openhuman_voice_input");
    tokio::fs::create_dir_all(&voice_dir)
        .await
        .map_err(|e| format!("Failed to create voice input directory: {e}"))?;

    let filename = format!(
        "voice-{}-{}.{}",
        Utc::now().timestamp_millis(),
        uuid::Uuid::new_v4(),
        ext
    );
    let file_path = voice_dir.join(filename);
    tokio::fs::write(&file_path, audio_bytes)
        .await
        .map_err(|e| format!("Failed to write audio file: {e}"))?;

    let output =
        local_ai::service::transcribe(&service, config, file_path.to_string_lossy().as_ref()).await;
    let _ = tokio::fs::remove_file(&file_path).await;

    let output = output.map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai transcription completed",
    ))
}

/// Performs text-to-speech synthesis and optionally saves the result to a file.
pub async fn local_ai_tts(
    config: &Config,
    text: &str,
    output_path: Option<&str>,
) -> Result<RpcOutcome<LocalAiTtsResult>, String> {
    let service = local_ai::global(config);
    let output = local_ai::service::tts(&service, config, text.trim(), output_path)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(output, "local ai tts completed"))
}

/// Returns the status of all local AI assets (models and support files).
pub async fn local_ai_assets_status(
    config: &Config,
) -> Result<RpcOutcome<LocalAiAssetsStatus>, String> {
    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let output = service
        .assets_status(&runtime)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai assets status fetched",
    ))
}

/// Returns progress for any ongoing asset downloads.
pub async fn local_ai_downloads_progress(
    config: &Config,
) -> Result<RpcOutcome<LocalAiDownloadsProgress>, String> {
    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let output = service
        .downloads_progress(&runtime)
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai downloads progress fetched",
    ))
}

/// Triggers the download of a specific AI asset based on capability name.
pub async fn local_ai_download_asset(
    config: &Config,
    capability: &str,
) -> Result<RpcOutcome<LocalAiAssetsStatus>, String> {
    let service = local_ai::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    let output = service
        .download_asset(&runtime, capability.trim())
        .await
        .map_err(|e| e.to_string())?;
    Ok(RpcOutcome::single_log(
        output,
        "local ai asset download triggered",
    ))
}
