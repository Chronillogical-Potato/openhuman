//! Where the session host's tool-result artifact store comes from (#6408).
//!
//! Extracted from `runtime_session.rs` rather than inlined there: the choice of
//! root is the whole correctness question for this feature, and it needs more
//! prose than a call site should carry.

use std::path::{Path, PathBuf};

use crate::agent::harness::tool_result_artifacts::ToolResultArtifactStore;

/// How long another session's tool-result artifacts survive before a later
/// session sweeps them.
///
/// **A conservative default chosen for safety, not a retention policy.** The
/// artifact store had no bound at all — nothing anywhere deletes these files —
/// and shipping unbounded growth in the user's action workspace to fix a
/// token-burn bug would trade one problem for another. This is the smallest
/// thing that cannot grow without limit; the feature's owner should confirm or
/// replace it.
///
/// Alternatives considered: a count cap (needs a policy for which artifacts are
/// worth keeping, which this code has no basis to decide), and a sweep at
/// session end (there is no single point where a session host ends, and a crash
/// would skip it entirely). Age is the only one of the three that is correct
/// without knowing the workload.
///
/// 24 hours because an artifact is only useful while the run that produced it
/// can still read it back, which is bounded by a turn — a day is already far
/// more generous than that window, and leaves a working day of artifacts
/// available for debugging a session after the fact.
const ARTIFACT_RETENTION: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

/// Where artifacts are written, chosen to match where the READ path will look.
///
/// The store writes under `<root>/artifacts/tool-results/…` but hands the model
/// a RELATIVE pointer, which `file_read` later resolves through
/// `security_for_tool_context` (`tools/impl/filesystem/mod.rs`). That function
/// overwrites `action_dir` with `ctx.workspace().root` whenever the turn carries
/// a workspace descriptor — and `turn()` sets `context.workspace` from the
/// host's own descriptor. So on a turn bound to a per-turn workspace, rooting
/// the store at `action_dir` writes artifacts the model cannot dereference: a
/// pointer to nothing, which is strictly worse than the inline truncation it
/// replaces, since that at least returns the head.
///
/// The descriptor is only `Some` when the embedder set a per-turn root, and in
/// its absence the policy's own `action_dir` survives the read path untouched.
/// Hence the same conditional on both sides rather than either one
/// unconditionally — pinning one would break the other case in the other
/// direction. Credit to the #6483 investigation for identifying the read-time
/// override; this is its session-host counterpart.
fn artifact_root(
    workspace_descriptor: Option<&tinytools::WorkspaceDescriptor>,
    action_dir: &Path,
) -> PathBuf {
    workspace_descriptor
        .map(|descriptor| descriptor.root.clone())
        .unwrap_or_else(|| action_dir.to_path_buf())
}

/// Build the store the turn path hands to `TurnContextMiddleware`, sweeping
/// other sessions' stale artifacts on the way.
///
/// The sweep is best-effort by design: a failed prune must never fail a turn,
/// because the worst it costs is disk, while failing the turn costs the user
/// their message.
pub(super) fn build_artifact_store(
    workspace_descriptor: Option<&tinytools::WorkspaceDescriptor>,
    action_dir: &Path,
    session_key: &str,
) -> ToolResultArtifactStore {
    let store =
        ToolResultArtifactStore::new(artifact_root(workspace_descriptor, action_dir), session_key);
    match store.prune_stale_sessions(ARTIFACT_RETENTION) {
        Ok(0) => {}
        Ok(removed) => log::debug!(
            "[agent][tool-result-artifacts] pruned {removed} stale artifact session dir(s)"
        ),
        Err(error) => {
            log::warn!("[agent][tool-result-artifacts] artifact prune failed (continuing): {error}")
        }
    }
    store
}
