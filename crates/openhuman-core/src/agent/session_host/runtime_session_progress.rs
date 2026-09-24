//! Post-commit progress delivery for a chat turn.

use crate::agent::progress::AgentProgress;

/// Preserve the response path if a progress receiver stays open but stops
/// consuming events. The web bridge has its own bounded drain wait afterward.
const COMMITTED_TURN_PROGRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

/// Send the terminal event as the progress bridge's drain fence. Content
/// capture stays best effort; the completion waits for channel capacity, up to
/// a deadline, so a healthy bridge can forward queued tool events first.
pub(super) async fn send_committed_turn_progress(
    progress: &tokio::sync::mpsc::Sender<AgentProgress>,
    input: &str,
    output: &str,
    iterations: u32,
) -> bool {
    let _ = progress.try_send(AgentProgress::TurnContent {
        input: Some(input.to_string()),
        output: Some(output.to_string()),
    });
    match tokio::time::timeout(
        COMMITTED_TURN_PROGRESS_TIMEOUT,
        progress.send(AgentProgress::TurnCompleted { iterations }),
    )
    .await
    {
        Ok(Ok(())) => true,
        Ok(Err(_)) => {
            log::warn!(
                "[agent_session] committed turn completion not delivered: progress receiver closed"
            );
            false
        }
        Err(_) => {
            log::warn!(
                "[agent_session] committed turn completion not delivered within {:?}: progress receiver stalled",
                COMMITTED_TURN_PROGRESS_TIMEOUT
            );
            false
        }
    }
}
