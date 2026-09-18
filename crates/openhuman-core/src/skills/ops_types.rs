//! OpenHuman-compatible names for the portable `tinyskills` model.

pub use tinyskills::model::{
    MAX_DESCRIPTION_LEN, MAX_NAME_LEN, RESOURCE_DIRS, SKILL_JSON, SKILL_MD, SKILL_TOML,
    WORKFLOW_MD, WORKFLOW_TOML,
};
pub use tinyskills::{
    Skill as Workflow, SkillFrontmatter as WorkflowFrontmatter, SkillScope as WorkflowScope,
};

pub(crate) const TRUST_MARKER: &str = "trust";
pub const MAX_WORKFLOW_RESOURCE_BYTES: u64 = tinyskills::model::MAX_RESOURCE_BYTES;

#[cfg(test)]
#[path = "ops_types_tests.rs"]
mod ops_types_tests;
