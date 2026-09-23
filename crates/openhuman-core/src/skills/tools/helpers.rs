//! Shared argument-parsing and skill-allowlist helpers for the workflow tools
//! in [`super::read`] and [`super::write`].

pub(in crate::skills) fn read_required_str(
    args: &serde_json::Value,
    key: &str,
) -> anyhow::Result<String> {
    args.get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .ok_or_else(|| anyhow::anyhow!("missing required string argument `{key}`"))
}

/// Read the target workflow id, accepting the legacy `skill_id` key as an
/// alias for `workflow_id` so callers from before the rename still work.
pub(in crate::skills) fn read_workflow_id(args: &serde_json::Value) -> anyhow::Result<String> {
    read_required_str(args, "workflow_id")
        .or_else(|_| read_required_str(args, "skill_id"))
        .map_err(|_| anyhow::anyhow!("missing required string argument `workflow_id`"))
}
