use crate::memory::agent::memory_loader::MemoryCitation;

pub fn segment_for_delivery_for_test(text: &str) -> Vec<String> {
    super::segment_for_delivery(text)
}

pub fn segment_delay_for_test(segment: &str) -> u64 {
    super::segment_delay(segment)
}

pub fn is_structured_content_for_test(text: &str) -> bool {
    super::is_structured_content(text)
}

pub async fn deliver_response_for_test(
    client_id: &str,
    thread_id: &str,
    request_id: &str,
    full_response: &str,
    user_message: &str,
    citations: &[MemoryCitation],
) {
    deliver_response_in_workspace_for_test(
        client_id,
        thread_id,
        request_id,
        full_response,
        user_message,
        citations,
        None,
    )
    .await;
}

/// `deliver_response` with an explicit `timing` snapshot and usage, so a
/// test can assert `chat_done.timing` (and its `tokens_per_second`
/// derivation) reaches the wire event.
pub async fn deliver_response_with_timing_for_test(
    client_id: &str,
    thread_id: &str,
    request_id: &str,
    full_response: &str,
    user_message: &str,
    usage: Option<&super::LastTurnUsage>,
    timing: Option<super::super::turn_timing::TurnTimingSnapshot>,
) {
    super::deliver_response(
        client_id,
        thread_id,
        request_id,
        full_response,
        user_message,
        &[],
        usage,
        None,
        timing,
        false,
    )
    .await;
}

/// `deliver_response` with an explicit workspace, so a test can assert the
/// reply reached disk before the turn was announced (#6034).
pub async fn deliver_response_in_workspace_for_test(
    client_id: &str,
    thread_id: &str,
    request_id: &str,
    full_response: &str,
    user_message: &str,
    citations: &[MemoryCitation],
    workspace_dir: Option<&std::path::Path>,
) {
    super::deliver_response(
        client_id,
        thread_id,
        request_id,
        full_response,
        user_message,
        citations,
        None,
        workspace_dir,
        None,
        // Test helper: suggestions are exercised directly against
        // `web_chat::suggestions`, not through this shared delivery path.
        false,
    )
    .await;
}
