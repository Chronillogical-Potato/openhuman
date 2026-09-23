//! Compatibility facade over the portable `tinyskills` document parser.

use std::path::Path;

use super::ops_types::{Workflow, WorkflowFrontmatter, WorkflowScope};

pub use tinyskills::inventory_resources;

pub fn parse_workflow_md(path: &Path) -> Option<(WorkflowFrontmatter, String, Vec<String>)> {
    tinyskills::parse_skill(path)
}

pub fn parse_workflow_md_str(content: &str) -> Option<(WorkflowFrontmatter, String, Vec<String>)> {
    tinyskills::parse_skill_str(content)
}

pub(crate) fn load_from_workflow_md(
    skill_md: &Path,
    dir: &Path,
    dir_name: &str,
    scope: WorkflowScope,
) -> Workflow {
    let mut workflow = tinyskills::document::load_document(skill_md, dir, dir_name, scope);
    if workflow.source_format == "agentskills" {
        workflow.source_format = "openhuman".to_string();
    }
    workflow
}

pub(crate) fn load_from_legacy_manifest(
    manifest_path: &Path,
    dir: &Path,
    dir_name: &str,
    scope: WorkflowScope,
) -> Workflow {
    tinyskills::document::load_legacy(manifest_path, dir, dir_name, scope)
}
