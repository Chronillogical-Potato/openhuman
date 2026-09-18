//! OpenHuman discovery context around `tinyskills` resource reads.

use std::collections::HashSet;
use std::path::{Component, Path};

use crate::skills::ops_types::WorkflowScope;

use super::api::load_workflow_metadata_for_profile;
use super::scan::scan_root;

fn reject_symlink_components(root: &Path, relative_path: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative_path.components() {
        current.push(component.as_os_str());
        let metadata = std::fs::symlink_metadata(&current)
            .map_err(|error| format!("failed to stat resource {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err("resource path is a symlink".to_owned());
        }
    }
    Ok(())
}

fn validate_relative_path(relative_path: &Path) -> Result<(), String> {
    if relative_path.as_os_str().is_empty() {
        return Err("relative_path must not be empty".to_owned());
    }
    if relative_path.is_absolute() {
        return Err("relative_path must not be absolute".to_owned());
    }
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("relative_path must not contain '..' or escape the skill directory".to_owned());
    }
    Ok(())
}

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
    validate_relative_path(relative_path)?;
    let skill = tinyskills::resolve_skill(
        load_workflow_metadata_for_profile(workspace_dir, profile_skills_root),
        skill_id,
    )?;
    let root = skill
        .location
        .as_deref()
        .and_then(Path::parent)
        .ok_or_else(|| format!("skill '{skill_id}' has no on-disk location"))?;
    reject_symlink_components(root, relative_path)?;
    tinyskills::read_resource(&skill, relative_path)
}
