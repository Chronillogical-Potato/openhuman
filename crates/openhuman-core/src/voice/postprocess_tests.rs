use super::*;

// Host wrapper: config flag and delegation. LLM-call behavior (ready/degraded,
// empty reply, error, timeout, context prompt) is tested in tinyinference-voice.

#[tokio::test]
async fn disabled_cleanup_returns_raw_text() {
    let _g = crate::inference::inference_test_guard_async().await;
    let mut config = Config::default();
    config.local_ai.voice_llm_cleanup_enabled = false;
    let service = local_ai::global(&config);
    let previous = service.replace_status_state("not_ready");
    let result = cleanup_transcription(&config, "um hello uh world", None).await;
    service.replace_status_state(previous);
    assert_eq!(result, "um hello uh world");
}

#[tokio::test]
async fn enabled_but_llm_not_ready_returns_raw_text() {
    // Covers the branch where cleanup is enabled in config but the
    // local LLM hasn't reached the ready/degraded state yet —
    // cleanup must gracefully fall back to the raw transcript.
    let _g = crate::inference::inference_test_guard_async().await;
    let config = Config::default(); // voice_llm_cleanup_enabled = true by default
    let service = local_ai::global(&config);
    let previous = service.replace_status_state("not_ready");
    let result = cleanup_transcription(&config, "raw whisper output", None).await;
    service.replace_status_state(previous);
    assert_eq!(result, "raw whisper output");
}
