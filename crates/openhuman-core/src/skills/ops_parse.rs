//! Compatibility facade over the portable `tinyskills` document parser.

use std::path::Path;

use super::ops_types::WorkflowFrontmatter;

pub use tinyskills::inventory_resources;

pub fn parse_workflow_md(path: &Path) -> Option<(WorkflowFrontmatter, String, Vec<String>)> {
    tinyskills::parse_skill(path)
}

pub fn parse_workflow_md_str(content: &str) -> Option<(WorkflowFrontmatter, String, Vec<String>)> {
    tinyskills::parse_skill_str(content)
}
