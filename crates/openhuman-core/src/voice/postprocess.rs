//! OpenHuman configuration adapter for transcription cleanup.

use std::time::Duration;

use crate::config::Config;
#[cfg(test)]
use crate::inference::host_runtime as local_ai;

/// Clean up a raw transcript through TinyInference's local voice pipeline.
pub async fn cleanup_transcription(
    config: &Config,
    raw_text: &str,
    conversation_context: Option<&str>,
) -> String {
    let service = crate::inference::host_runtime::global(config);
    let runtime = crate::inference::local_runtime_config(config);
    tinyinference_voice::postprocess::cleanup_transcription(
        service.as_ref(),
        &runtime,
        raw_text,
        conversation_context,
        Duration::from_secs(3),
    )
    .await
}

#[cfg(test)]
#[path = "postprocess_tests.rs"]
mod tests;
