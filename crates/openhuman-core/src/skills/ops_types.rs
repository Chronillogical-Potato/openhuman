//! OpenHuman-compatible names for the portable `tinyskills` model.

pub use tinyskills::{
    Skill as Workflow, SkillFrontmatter as WorkflowFrontmatter, SkillScope as WorkflowScope,
};

pub const MAX_DESCRIPTION_LEN: usize = 1024;
pub const MAX_NAME_LEN: usize = 64;
pub const RESOURCE_DIRS: &[&str] = &[
    "scripts",
    "references",
    "assets",
    "templates",
    "examples",
    "prompts",
];
pub const SKILL_JSON: &str = "skill.json";
pub const SKILL_MD: &str = "SKILL.md";
pub const SKILL_TOML: &str = "skill.toml";
pub const WORKFLOW_MD: &str = "WORKFLOW.md";
pub const WORKFLOW_TOML: &str = "workflow.toml";

pub(crate) const TRUST_MARKER: &str = "trust";
pub const MAX_WORKFLOW_RESOURCE_BYTES: u64 = 128 * 1024;

/// Read a string-valued metadata entry from skill frontmatter.
pub(crate) fn metadata_string(fm: &WorkflowFrontmatter, key: &str) -> Option<String> {
    fm.metadata
        .get(key)
        .and_then(|value| value.as_str())
        .map(str::to_owned)
}

/// Read string entries from a YAML sequence metadata value.
pub(crate) fn metadata_string_seq(value: &serde_yaml::Value) -> Vec<String> {
    value
        .as_sequence()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// Read version metadata, warning when the legacy top-level form is used.
pub(crate) fn extract_version(fm: &WorkflowFrontmatter, warnings: &mut Vec<String>) -> String {
    metadata_string(fm, "version")
        .or_else(|| {
            fm.extra
                .get("version")
                .and_then(|value| value.as_str())
                .map(|value| {
                    warnings.push(
                        "top-level 'version' is deprecated; move under 'metadata.version'"
                            .to_string(),
                    );
                    value.to_owned()
                })
        })
        .unwrap_or_default()
}

/// Read author metadata, warning when the legacy top-level form is used.
pub(crate) fn extract_author(
    fm: &WorkflowFrontmatter,
    warnings: &mut Vec<String>,
) -> Option<String> {
    metadata_string(fm, "author").or_else(|| {
        fm.extra
            .get("author")
            .and_then(|value| value.as_str())
            .map(|value| {
                warnings.push(
                    "top-level 'author' is deprecated; move under 'metadata.author'".to_string(),
                );
                value.to_owned()
            })
    })
}

/// Read and normalize tags from current and legacy metadata locations.
pub(crate) fn extract_tags(fm: &WorkflowFrontmatter, warnings: &mut Vec<String>) -> Vec<String> {
    let mut tags = fm
        .metadata
        .get("tags")
        .map(metadata_string_seq)
        .unwrap_or_default();
    if let Some(value) = fm
        .metadata
        .get("hermes")
        .and_then(|value| value.as_mapping())
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("tags".to_string())))
    {
        tags.extend(metadata_string_seq(value));
    }
    if let Some(value) = fm.extra.get("tags") {
        warnings.push("top-level 'tags' is deprecated; move under 'metadata.tags'".to_string());
        tags.extend(metadata_string_seq(value));
    }
    tags.sort();
    tags.dedup();
    tags
}

/// Read related-skill metadata from current and Hermes-compatible locations.
pub(crate) fn extract_related_skills(fm: &WorkflowFrontmatter) -> Vec<String> {
    let mut related = fm
        .metadata
        .get("related_skills")
        .map(metadata_string_seq)
        .unwrap_or_default();
    if let Some(value) = fm
        .metadata
        .get("hermes")
        .and_then(|value| value.as_mapping())
        .and_then(|mapping| mapping.get(serde_yaml::Value::String("related_skills".to_string())))
    {
        related.extend(metadata_string_seq(value));
    }
    related.sort();
    related.dedup();
    related
}

/// Identify the source ecosystem represented by frontmatter.
pub(crate) fn detect_source_format(fm: &WorkflowFrontmatter) -> String {
    if fm.metadata.contains_key("hermes") || !fm.platforms.is_empty() {
        "hermes".to_string()
    } else {
        "openhuman".to_string()
    }
}

/// Compatibility shape for callers that deserialize legacy `skill.json` files.
#[derive(Debug, serde::Deserialize)]
pub(crate) struct LegacyWorkflowManifest {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub prompts: Vec<String>,
}

#[cfg(test)]
#[path = "ops_types_tests.rs"]
mod ops_types_tests;
