use super::OpenHumanSessionHost;

/// Stage direct user input against the exact runtime and workspace bound to
/// this session. Library callers with no explicit origin retain their existing
/// authorized attachment behavior.
pub(super) async fn stage_turn(
    host: &OpenHumanSessionHost,
    message: &str,
    origin: Option<&crate::agent::turn_origin::AgentTurnOrigin>,
) -> anyhow::Result<String> {
    crate::agent::attachments::stage_turn(
        message,
        host.runtime_config.as_deref(),
        host.workspace_descriptor.as_ref(),
        host.thread_id.as_deref(),
        origin,
    )
    .await
}
