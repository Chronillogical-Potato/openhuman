//! OpenHuman discovery context around `tinyskills` resource reads.

use std::collections::HashSet;
use std::path::Path;

use crate::skills::ops_types::WorkflowScope;

use super::api::load_workflow_metadata_for_profile;
use super::scan::scan_root;

pub fn read_workflow_resource(
    workspace_dir: &Path,
    skill_id: &str,
    relative_path: &Path,
) -> Result<String, String> {
    read_workflow_resource_with_profile(workspace_dir, skill_id, relative_path, None)
}

pub fn profile_local_skill_ids(profile_skills_root: Option<&Path>) -> HashSet<String> {
    let Some(root) = profile_skills_root else {
        return HashSet::new();
    };
    scan_root(root, WorkflowScope::Profile)
        .into_iter()
        .flat_map(|workflow| [workflow.name, workflow.dir_name])
        .filter(|id| !id.is_empty())
        .collect()
}

pub fn read_workflow_resource_with_profile(
    workspace_dir: &Path,
    skill_id: &str,
    relative_path: &Path,
    profile_skills_root: Option<&Path>,
) -> Result<String, String> {
    if skill_id.trim().is_empty() {
        return Err("skill_id must not be empty".to_string());
    }
    let skill = tinyskills::resolve_skill(
        load_workflow_metadata_for_profile(workspace_dir, profile_skills_root),
        skill_id,
    )?;
    tinyskills::read_resource(&skill, relative_path)
}
