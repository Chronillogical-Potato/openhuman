//! Tauri commands for exporting agent-generated artifacts (#2779).
//!
//! One export path, fed by an artifact id:
//! [`download_artifact_to_downloads`] resolves the id through the core's
//! artifact store, copies the file into the user's Downloads directory with a
//! non-colliding name, and returns the dest path so the UI can offer "Reveal in
//! Finder". Cross-platform.
//!
//! **The native Save-As dialog (#3162) was removed.** It was one call into
//! `rfd`, and `rfd` carried 13 packages — the xdg-desktop-portal client
//! (`ashpd`), `zbus`, and the `async-io`/`polling` executor stack — into a
//! binary that already reaches D-Bus through other paths. The frontend's
//! `saveArtifactViaDialog` had a Downloads fallback for hosts with no
//! portal from the day it landed, so that fallback is simply the only path
//! now; the user still gets the file plus "Reveal in Finder", one dialog
//! fewer. If a real Save-As is wanted again, prefer the destination-picking
//! surface Tauri itself already links over re-adding a second dialog stack.
//!
//! The renderer names an artifact, never a path (#5505). Artifact files now
//! live in a visible, user-configurable folder rather than under the data
//! dir, so the shell cannot check a renderer-supplied path against a fixed
//! root; instead it asks the core, whose `resolve_ready_file` applies the
//! artifact escape guard and refuses records that are not `Ready`. A
//! compromised renderer can therefore only copy out files the artifact store
//! already vouches for. The filename hint is still sanitized so nothing is
//! written outside the Downloads directory.

use std::path::{Path, PathBuf};

/// Resolve an artifact id to the file the core's artifact store vouches for,
/// in the given workspace. Isolated from config loading for unit testing.
async fn resolve_source(
    workspace_dir: &Path,
    roots: &openhuman_core::agent::artifacts::FileRoots,
    artifact_id: &str,
) -> Result<PathBuf, String> {
    let artifact_id = artifact_id.trim();
    if artifact_id.is_empty() {
        return Err("artifact_id must not be empty".to_string());
    }
    openhuman_core::agent::artifacts::resolve_ready_file(workspace_dir, roots, artifact_id).await
}

/// Copy `source` to `dest`, returning the byte count. Isolated so it is
/// unit-testable without touching the real Downloads directory.
async fn copy_to_path(source: &Path, dest: &Path) -> Result<u64, String> {
    tokio::fs::copy(source, dest)
        .await
        .map_err(|e| format!("failed to copy artifact to {:?}: {e}", dest))
}

/// Maximum number of `(N)` suffixes we'll append when picking a
/// non-colliding filename. After 1000 we give up and append a UUID
/// suffix instead so the download never silently overwrites.
const MAX_COLLISION_SUFFIX: u32 = 1000;

#[tauri::command]
pub async fn download_artifact_to_downloads(
    artifact_id: String,
    filename: String,
) -> Result<String, String> {
    if artifact_id.trim().is_empty() {
        return Err("artifact_id must not be empty".to_string());
    }
    let config = openhuman_core::config::rpc::load_config_with_timeout().await?;
    let roots = openhuman_core::agent::artifacts::FileRoots::from_config(&config);
    let source = resolve_source(&config.workspace_dir, &roots, &artifact_id).await?;
    if filename.trim().is_empty() {
        return Err("filename must not be empty".to_string());
    }
    let sanitized = sanitize_filename(&filename)?;

    let downloads = directories::UserDirs::new()
        .and_then(|u| u.download_dir().map(|p| p.to_path_buf()))
        .ok_or_else(|| "OS Downloads directory not resolvable".to_string())?;
    tokio::fs::create_dir_all(&downloads)
        .await
        .map_err(|e| format!("failed to ensure Downloads dir {:?}: {e}", downloads))?;

    let dest = pick_unique_path(&downloads, &sanitized);
    let bytes = copy_to_path(&source, &dest).await?;

    log::info!(
        "[artifact_commands] download_artifact_to_downloads bytes={bytes} dest={}",
        dest.display()
    );
    Ok(dest.display().to_string())
}

/// Strip path-traversal characters from a filename hint. The
/// renderer is expected to pass something like `"My Deck.pptx"`;
/// reject anything that contains a separator or null byte so a
/// malicious `ai_get_artifact` response can never escape the chosen dir.
fn sanitize_filename(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("filename must not be empty after trim".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') {
        return Err(format!(
            "filename must not contain path separators: {trimmed:?}"
        ));
    }
    if trimmed.contains('\0') {
        return Err(format!("filename must not contain NUL bytes: {trimmed:?}"));
    }
    if trimmed == "." || trimmed == ".." {
        return Err(format!("filename must not be '.' or '..': {trimmed:?}"));
    }
    Ok(trimmed.to_string())
}

/// Pick a destination path under `dir` that does not exist yet.
/// Inserts ` (N)` between the stem and the extension. Falls back to
/// a UUID suffix after [`MAX_COLLISION_SUFFIX`] tries.
fn pick_unique_path(dir: &Path, filename: &str) -> PathBuf {
    let candidate = dir.join(filename);
    if !candidate.exists() {
        return candidate;
    }
    let (stem, ext) = split_stem_ext(filename);
    for n in 1..=MAX_COLLISION_SUFFIX {
        let nth = if ext.is_empty() {
            format!("{stem} ({n})")
        } else {
            format!("{stem} ({n}).{ext}")
        };
        let path = dir.join(&nth);
        if !path.exists() {
            return path;
        }
    }
    // 1000 collisions is implausible in practice; if we hit it, fall
    // back to a monotonic nanosecond suffix so the copy still succeeds
    // without overwriting anything. Reaches for the OS clock instead of
    // pulling in `uuid` as a Tauri-shell dep just for this corner.
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let with_uniq = if ext.is_empty() {
        format!("{stem}-{nanos}")
    } else {
        format!("{stem}-{nanos}.{ext}")
    };
    dir.join(with_uniq)
}

fn split_stem_ext(filename: &str) -> (String, String) {
    if let Some(idx) = filename.rfind('.') {
        // Reject leading-dot files (`.hidden`) — treat as having no extension.
        if idx > 0 && idx < filename.len() - 1 {
            return (filename[..idx].to_string(), filename[idx + 1..].to_string());
        }
    }
    (filename.to_string(), String::new())
}

#[cfg(test)]
#[path = "artifact_commands_tests.rs"]
mod tests;
