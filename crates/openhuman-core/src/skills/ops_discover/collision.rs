//! Cross-scope collision resolution for discovered skills.

use std::collections::HashMap;

use crate::skills::ops_types::{Workflow, WorkflowScope};

pub(super) fn absorb(by_name: &mut HashMap<String, Workflow>, incoming: Vec<Workflow>) {
    for mut skill in incoming {
        if !is_supported_scope(skill.scope) {
            continue;
        }
        let key = skill.name.clone();
        let collision_keys: Vec<String> = by_name
            .iter()
            .filter(|(existing_name, existing)| {
                existing_name.as_str() == key || existing.dir_name == skill.dir_name
            })
            .map(|(existing_name, _)| existing_name.clone())
            .collect();

        let highest = collision_keys
            .iter()
            .filter_map(|collision_key| by_name.get(collision_key))
            .map(|existing| (existing.name.clone(), existing.scope))
            .max_by_key(|(_, scope)| precedence(*scope));
        if let Some((highest_name, highest_scope)) = highest {
            if precedence(skill.scope) >= precedence(highest_scope) {
                // The incoming skill wins ties, preserving the historical
                // last-writer behavior for equal-precedence roots.
            } else {
                if let Some(kept) = by_name.get_mut(&highest_name) {
                    kept.warnings.push(format!(
                        "workflow id '{}' or name '{}' also declared in {:?} scope at {} (ignored)",
                        skill.dir_name,
                        skill.name,
                        skill.scope,
                        skill.location.as_deref().map_or_else(
                            || "<unknown>".to_owned(),
                            |path| path.display().to_string()
                        )
                    ));
                }
                continue;
            }
        }

        for collision_key in collision_keys {
            if let Some(loser) = by_name.remove(&collision_key) {
                skill.warnings.push(format!(
                    "shadowed {:?}-scope skill '{}' (workflow id '{}') at {}",
                    loser.scope,
                    loser.name,
                    loser.dir_name,
                    loser
                        .location
                        .as_deref()
                        .map_or_else(|| "<unknown>".to_owned(), |path| path.display().to_string())
                ));
            }
        }
        by_name.insert(key, skill);
    }
}

pub(super) const fn precedence(scope: WorkflowScope) -> u8 {
    match scope {
        WorkflowScope::Builtin => 0,
        WorkflowScope::Legacy => 1,
        WorkflowScope::User => 2,
        WorkflowScope::Project => 3,
        WorkflowScope::Flow => 4,
        _ => 0,
    }
}

const fn is_supported_scope(scope: WorkflowScope) -> bool {
    matches!(
        scope,
        WorkflowScope::Builtin
            | WorkflowScope::Legacy
            | WorkflowScope::User
            | WorkflowScope::Project
            | WorkflowScope::Flow
    )
}
