//! `apply_patch` — atomic multi-edit across one or more files.
//!
//! Coding-harness baseline tool (issue #1205). Takes an array of
//! `{path, old_string, new_string}` edits and applies them atomically:
//! every edit is validated up front (path, exact-match, uniqueness)
//! before any file is written. If any edit fails validation, no files
//! are touched.
//!
//! **Creating a file** is an empty `old_string` against a path that does not
//! exist yet; `new_string` becomes the whole contents. Before that existed the
//! tool could only edit, and an agent asked to produce a document had no route
//! at all: it would create a placeholder with `shell` purely so there was
//! something to patch (the life-scenario benchmark caught two 1-byte files
//! containing `x`). An empty `old_string` against a file that *does* exist is
//! still an error — "replace nothing" is ambiguous, not a create.

use crate::agent::file_state;
use crate::security::{CommandClass, GateDecision, SecurityPolicy};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tinytools::ToolRunContext;
use tinytools::{PermissionLevel, Tool, ToolCallOptions, ToolResult};

const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_EDITS: usize = 50;

pub struct ApplyPatchTool {
    security: Arc<SecurityPolicy>,
}

impl ApplyPatchTool {
    pub fn new(security: Arc<SecurityPolicy>) -> Self {
        Self { security }
    }
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &str {
        "apply_patch"
    }

    fn description(&self) -> &str {
        "Apply a batch of exact-string edits across one or more files atomically. \
         All edits are validated before any are written; validation failure rolls \
         back the whole batch. Each edit is `{path, old_string, new_string, replace_all?}`. \
         To CREATE a new file, pass an empty `old_string` with the full contents \
         as `new_string`; the path must not already exist."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "edits": {
                    "type": "array",
                    "description": "Ordered list of edits.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": { "type": "string" },
                            "old_string": {
                                "type": "string",
                                "description": "Exact text to replace. Empty means CREATE: the path must not exist and `new_string` becomes the whole file."
                            },
                            "new_string": { "type": "string" },
                            "replace_all": { "type": "boolean", "default": false }
                        },
                        "required": ["path", "old_string", "new_string"]
                    }
                }
            },
            "required": ["edits"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    /// `apply_patch` modifies existing files → in ask-before-edit it routes
    /// through the human approval gate; in Full it runs; read-only is blocked
    /// in `execute`.
    fn external_effect_with_args(&self, _args: &serde_json::Value) -> bool {
        self.security.gate_decision(CommandClass::Write) == GateDecision::Prompt
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        self.execute_in_context(args, None).await
    }

    async fn execute_with_context(
        &self,
        args: serde_json::Value,
        _options: ToolCallOptions,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        self.execute_in_context(args, context).await
    }
}

impl ApplyPatchTool {
    async fn execute_in_context(
        &self,
        args: serde_json::Value,
        context: Option<&dyn ToolRunContext>,
    ) -> anyhow::Result<ToolResult> {
        let edits = args
            .get("edits")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow::anyhow!("Missing 'edits' array"))?;
        if edits.is_empty() {
            return Ok(ToolResult::error("`edits` array is empty"));
        }
        if edits.len() > MAX_EDITS {
            return Ok(ToolResult::error(format!(
                "Too many edits: {} (max {MAX_EDITS})",
                edits.len()
            )));
        }

        if !self.security.can_act() {
            return Ok(ToolResult::error(
                "[policy-blocked] Action blocked: autonomy is read-only",
            ));
        }
        if self.security.is_rate_limited() {
            return Ok(ToolResult::error(
                "Rate limit exceeded: too many actions in the last hour",
            ));
        }
        if !self.security.record_action() {
            return Ok(ToolResult::error(
                "Rate limit exceeded: action budget exhausted",
            ));
        }

        let path_policy = super::security_for_tool_context(&self.security, context, "apply_patch");

        // Parse + group edits by file.
        let mut parsed: Vec<ParsedEdit> = Vec::with_capacity(edits.len());
        for (i, raw) in edits.iter().enumerate() {
            let path = raw
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("edit[{i}]: missing `path`"))?;
            let old_string = raw
                .get("old_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("edit[{i}]: missing `old_string`"))?;
            let new_string = raw
                .get("new_string")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow::anyhow!("edit[{i}]: missing `new_string`"))?;
            let replace_all = raw
                .get("replace_all")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if !path_policy.is_path_string_allowed(path) {
                return Ok(ToolResult::error(format!(
                    "edit[{i}]: path not allowed: {path}"
                )));
            }
            // An empty `old_string` is a create, and only against a path that
            // does not exist yet. Resolved the same way every other edit is —
            // joined onto `action_dir` — so "exists" means the same thing here
            // as it does in the apply loop below.
            let exists = path_policy.action_dir.join(path).exists()
                || (std::path::Path::new(path).is_absolute()
                    && std::path::Path::new(path).exists());
            let create = old_string.is_empty();
            if create && exists {
                return Ok(ToolResult::error(format!(
                    "edit[{i}]: `old_string` must not be empty for an existing file ({path}). \
                     Pass the exact text to replace, or write to a new path to create a file."
                )));
            }
            parsed.push(ParsedEdit {
                index: i,
                path: path.to_string(),
                old_string: old_string.to_string(),
                new_string: new_string.to_string(),
                replace_all,
                create,
            });
        }

        // Acquire per-path locks for all unique paths before any reads.
        let unique_paths: Vec<String> = {
            let mut seen = std::collections::HashSet::new();
            parsed
                .iter()
                .filter_map(|e| {
                    if seen.insert(e.path.clone()) {
                        Some(e.path.clone())
                    } else {
                        None
                    }
                })
                .collect()
        };
        let mut _path_guards = Vec::new();
        for p in &unique_paths {
            let full = path_policy.action_dir.join(p);
            if let Ok(resolved) = tokio::fs::canonicalize(&full).await {
                if let Some(guard) = file_state::acquire_path_lock(&resolved).await {
                    _path_guards.push(guard);
                }
            }
        }

        // File-state guard: reject edits based on stale or partial reads.
        if let Some(agent_id) = file_state::current_file_state_agent_id() {
            for p in &unique_paths {
                let full = path_policy.action_dir.join(p);
                if let Ok(resolved) = tokio::fs::canonicalize(&full).await {
                    if let Some(msg) = file_state::check_stale_read(&agent_id, &resolved) {
                        tracing::debug!(
                            agent = %agent_id,
                            path = %resolved.display(),
                            "[file_state] apply_patch blocked: stale read"
                        );
                        return Ok(ToolResult::error(msg));
                    }
                    if let Some(msg) = file_state::check_partial_read(&agent_id, &resolved) {
                        tracing::debug!(
                            agent = %agent_id,
                            path = %resolved.display(),
                            "[file_state] apply_patch blocked: partial read"
                        );
                        return Ok(ToolResult::error(msg));
                    }
                }
            }
        }

        // Resolve paths + load file contents (once per file). Apply edits in
        // memory; if any edit fails, return without writing.
        let mut buffers: HashMap<String, FileBuffer> = HashMap::new();
        for edit in &parsed {
            if !buffers.contains_key(&edit.path) {
                let full = path_policy.action_dir.join(&edit.path);

                // Symlink check must happen on the *unresolved* path —
                // canonicalize resolves symlinks, so a check after that
                // point would never see the link.
                if let Ok(meta) = tokio::fs::symlink_metadata(&full).await {
                    if meta.file_type().is_symlink() {
                        return Ok(ToolResult::error(format!(
                            "edit[{}]: refusing to edit through symlink",
                            edit.index
                        )));
                    }
                }

                // Security check: validate path string, resolve symlinks, confirm
                // workspace containment. A create has no file to canonicalize,
                // so it resolves through the parent — the same gate
                // `file_write` uses, which walks up to the deepest existing
                // ancestor and still checks it for symlink escapes.
                let resolved = if edit.create {
                    match path_policy.validate_parent_path(&edit.path).await {
                        Ok(p) => p,
                        Err(msg) => {
                            return Ok(ToolResult::error(format!("edit[{}]: {msg}", edit.index)));
                        }
                    }
                } else {
                    match path_policy.validate_path(&edit.path).await {
                        Ok(p) => p,
                        Err(msg) => {
                            return Ok(ToolResult::error(format!("edit[{}]: {msg}", edit.index)));
                        }
                    }
                };
                if edit.create {
                    if let Some(parent) = resolved.parent() {
                        if let Err(e) = tokio::fs::create_dir_all(parent).await {
                            return Ok(ToolResult::error(format!(
                                "edit[{}]: failed to create parent of {}: {e}",
                                edit.index, edit.path
                            )));
                        }
                    }
                    buffers.insert(
                        edit.path.clone(),
                        FileBuffer {
                            resolved,
                            original: None,
                            contents: edit.new_string.clone(),
                            edit_count: 1,
                        },
                    );
                    continue;
                }
                if let Ok(meta) = tokio::fs::metadata(&resolved).await {
                    if meta.len() > MAX_FILE_BYTES {
                        return Ok(ToolResult::error(format!(
                            "edit[{}]: file too large ({} bytes)",
                            edit.index,
                            meta.len()
                        )));
                    }
                }
                let contents = match tokio::fs::read_to_string(&resolved).await {
                    Ok(c) => c,
                    Err(e) => {
                        return Ok(ToolResult::error(format!(
                            "edit[{}]: failed to read {}: {e}",
                            edit.index, edit.path
                        )));
                    }
                };
                buffers.insert(
                    edit.path.clone(),
                    FileBuffer {
                        resolved,
                        original: Some(contents.clone()),
                        contents,
                        edit_count: 0,
                    },
                );
            }

            if edit.create {
                // A second create for the same path in one batch. The first one
                // already populated the buffer; a duplicate would silently
                // discard one of the two bodies, so say so.
                return Ok(ToolResult::error(format!(
                    "edit[{}]: {} is created earlier in this batch; only one create per path",
                    edit.index, edit.path
                )));
            }

            let buf = buffers.get_mut(&edit.path).unwrap();
            let count = buf.contents.matches(&edit.old_string).count();
            if count == 0 {
                return Ok(ToolResult::error(format!(
                    "edit[{}]: `old_string` not found in {}",
                    edit.index, edit.path
                )));
            }
            if count > 1 && !edit.replace_all {
                return Ok(ToolResult::error(format!(
                    "edit[{}]: `old_string` matches {count} times in {}; pass `replace_all`",
                    edit.index, edit.path
                )));
            }
            buf.contents = if edit.replace_all {
                buf.contents.replace(&edit.old_string, &edit.new_string)
            } else {
                buf.contents.replacen(&edit.old_string, &edit.new_string, 1)
            };
            buf.edit_count += count;
        }

        // Best-effort atomic write across files. We cannot get true
        // multi-file atomicity without filesystem-level transactions,
        // but if the i-th write fails we attempt to restore originals
        // for the i-1 already-written files from the in-memory snapshot.
        let mut summary: Vec<String> = Vec::new();
        let mut written: Vec<&FileBuffer> = Vec::new();
        for (path, buf) in &buffers {
            if let Err(e) = tokio::fs::write(&buf.resolved, &buf.contents).await {
                let restore_errors = restore_originals(&written).await;
                let suffix = if restore_errors.is_empty() {
                    "; previously-written files restored from snapshot".to_string()
                } else {
                    format!("; restore failed for: {}", restore_errors.join(", "))
                };
                return Ok(ToolResult::error(format!(
                    "Failed to write {path}: {e}{suffix}"
                )));
            }
            written.push(buf);
            summary.push(match buf.original {
                None => format!("{path}: created ({} bytes)", buf.contents.len()),
                Some(_) => format!("{path}: {} replacement(s)", buf.edit_count),
            });
        }
        // Record writes in the file-state coordinator.
        if let Some(agent_id) = file_state::current_file_state_agent_id() {
            for buf in buffers.values() {
                file_state::record_write(&agent_id, buf.resolved.clone());
            }
        }

        summary.sort();
        Ok(ToolResult::success(format!(
            "Applied {} edit(s) across {} file(s)\n{}",
            parsed.len(),
            buffers.len(),
            summary.join("\n")
        )))
    }
}

async fn restore_originals(written: &[&FileBuffer]) -> Vec<String> {
    let mut errors = Vec::new();
    for buf in written {
        let result = match &buf.original {
            Some(original) => tokio::fs::write(&buf.resolved, original).await,
            // The batch created this file, so "restore" means remove it —
            // otherwise a failed multi-file patch leaves a half-written new
            // file behind and the caller cannot tell it apart from a success.
            None => tokio::fs::remove_file(&buf.resolved).await,
        };
        if let Err(e) = result {
            errors.push(format!("{}: {e}", buf.resolved.display()));
        }
    }
    errors
}

struct ParsedEdit {
    index: usize,
    path: String,
    old_string: String,
    new_string: String,
    replace_all: bool,
    /// Empty `old_string` against a path that does not exist: `new_string`
    /// becomes the whole file.
    create: bool,
}

struct FileBuffer {
    resolved: PathBuf,
    /// Snapshot of the file's contents as we first read them.
    /// Used to restore on a partial-write failure. `None` for a file this batch
    /// is creating — there is nothing to restore *to*, so rollback deletes it.
    original: Option<String>,
    contents: String,
    edit_count: usize,
}

#[cfg(test)]
#[path = "apply_patch_tests.rs"]
mod tests;
