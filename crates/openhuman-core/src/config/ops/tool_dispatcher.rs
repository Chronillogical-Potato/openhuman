//! Validation for the `[agent] tool_dispatcher` setting.

/// Accepted spellings of `[agent] tool_dispatcher`. `auto` is the default:
/// native structured calls when the provider supports them, else JSON-in-tag.
pub const TOOL_DISPATCHER_CHOICES: [&str; 6] =
    ["auto", "native", "xml", "pformat", "python", "typescript"];

/// Trim and lowercase `raw`, rejecting anything outside
/// [`TOOL_DISPATCHER_CHOICES`].
pub fn normalize_tool_dispatcher(raw: &str) -> Result<String, String> {
    let normalized = raw.trim().to_ascii_lowercase();
    if !TOOL_DISPATCHER_CHOICES.contains(&normalized.as_str()) {
        log::warn!("[config][agent] rejected tool_dispatcher={normalized:?}");
        return Err(format!(
            "invalid tool_dispatcher '{normalized}' (expected {})",
            TOOL_DISPATCHER_CHOICES.join(" | ")
        ));
    }
    Ok(normalized)
}

/// [`normalize_tool_dispatcher`] over an optional patch field.
pub fn normalize_optional(raw: Option<&str>) -> Result<Option<String>, String> {
    raw.map(normalize_tool_dispatcher).transpose()
}

/// Store a validated dispatcher on the agent config; `None` leaves it alone.
pub fn apply_tool_dispatcher(config: &mut crate::config::Config, value: Option<String>) {
    if let Some(value) = value {
        log::debug!("[config][agent] tool_dispatcher -> {value}");
        config.agent.tool_dispatcher = value;
    }
}

/// True when `OPENHUMAN_TOOL_DISPATCHER` overrides the persisted setting.
pub fn tool_dispatcher_env_override() -> bool {
    std::env::var("OPENHUMAN_TOOL_DISPATCHER")
        .map(|v| !v.trim().is_empty())
        .unwrap_or(false)
}
