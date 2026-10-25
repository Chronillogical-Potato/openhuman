use std::path::{Path, PathBuf};

use super::types::{SecurityPolicy, TrustedAccess};
use super::types::{
    ACCOUNT_CONFIG_FILE, ARTIFACTS_DIR, ARTIFACT_TOOL_RESULTS_DIR, WORKSPACE_INTERNAL_DIRS,
    WORKSPACE_INTERNAL_FILES,
};
use tinytools::status::POLICY_BLOCKED_MARKER;

/// Credential-store path segments, matched case-insensitively at any depth.
pub(crate) const SENSITIVE_COMPONENTS: &[&str] =
    &[".ssh", ".gnupg", ".aws", ".azure", ".kube", "keychains"];
/// Single-file credential stores have no protected parent directory.
pub(crate) const SENSITIVE_FILES: &[&str] = &[".netrc", ".git-credentials", ".pgpass", ".npmrc"];
/// Consecutive segment pairs that mark a credential store. Windows DPAPI
/// stores live under `...\Microsoft\{Protect,Credentials,Crypto,Vault}`; the
/// pair keeps the generic second name from matching an unrelated directory.
pub(crate) const SENSITIVE_PAIRS: &[[&str; 2]] = &[
    [".docker", "config.json"],
    [".cargo", "credentials.toml"],
    ["microsoft", "protect"],
    ["microsoft", "credentials"],
    ["microsoft", "crypto"],
    ["microsoft", "vault"],
];
/// The credential stores above as absolute paths under `home`, for a jail that
/// denies reads by path rather than by segment. macOS keeps keychains in
/// `~/Library/Keychains`. The Windows DPAPI pairs have no fixed place under a
/// home folder, so they stay with the segment check alone.
#[cfg(any(feature = "modules", test))]
pub(crate) fn home_credential_stores(home: &Path) -> Vec<PathBuf> {
    let components = SENSITIVE_COMPONENTS.iter().map(|segment| match *segment {
        "keychains" => home.join("Library").join("Keychains"),
        segment => home.join(segment),
    });
    let files = SENSITIVE_FILES.iter().map(|file| home.join(file));
    let pairs = SENSITIVE_PAIRS
        .iter()
        .filter(|[dir, _]| dir.starts_with('.'))
        .map(|[dir, file]| home.join(dir).join(file));
    components.chain(files).chain(pairs).collect()
}
/// Core OS directories matched by absolute (lower-cased) prefix. `/private` is
/// intentionally absent: macOS temp dirs canonicalize under `/private/var`.
pub(crate) const ALWAYS_FORBIDDEN_PREFIXES: &[&str] = &[
    "/etc",
    "/root",
    "/boot",
    "/proc",
    "/sys",
    "/system",
    "c:/windows",
    "c:/program files",
    "c:/program files (x86)",
    "c:/programdata",
];

impl SecurityPolicy {
    /// Expand a leading `~/` to the user's home directory. Delegates to
    /// [`crate::config::expand_tilde`] — the single source of truth —
    /// so policy and config expand paths byte-for-byte identically (and both
    /// produce platform-native separators; see issue #3353).
    pub(super) fn expand_tilde(&self, path: &str) -> String {
        crate::config::expand_tilde(path)
    }

    /// String-only path check. Does NOT resolve symlinks.
    /// Use `validate_path()` for any path that will be used for file I/O.
    pub fn is_path_string_allowed(&self, path: &str) -> bool {
        // Synchronous TinyTools preflight: only reject malformed path syntax.
        // Authorization (policy roots, protected floor, reserved state and
        // symlink resolution) is answered asynchronously by TinySecurity.
        !path.is_empty()
            && !path.contains('\0')
            && !Path::new(path)
                .components()
                .any(|component| matches!(component, std::path::Component::ParentDir))
            && !path.to_ascii_lowercase().contains("..%2f")
            && !path.to_ascii_lowercase().contains("%2f..")
    }

    fn try_canonicalize_under_workspace(&self, path: &str) -> Option<PathBuf> {
        let expanded = PathBuf::from(self.expand_tilde(path));
        let absolute = if expanded.is_absolute() {
            expanded
        } else {
            self.action_dir.join(&expanded)
        };
        if let Ok(canonical) = absolute.canonicalize() {
            return Some(canonical);
        }
        // Path itself does not exist (e.g. a write-to-new-file call). Try
        // canonicalizing the parent + appending the basename so we still
        // catch parent chains that resolve via symlink to a forbidden tree.
        let parent = absolute.parent()?;
        let name = absolute.file_name()?;
        parent.canonicalize().ok().map(|p| p.join(name))
    }

    /// Return the canonical form of `workspace_dir`, hydrating the
    /// `canonical_workspace` cache on the first call.
    ///
    /// `validate_path` / `validate_parent_path` both need the canonical
    /// workspace root for forbidden-path containment checks. The underlying
    /// `tokio::fs::canonicalize` is a `stat(2)` + symlink walk and was
    /// previously invoked on every call with the same input.
    ///
    /// Falls back to the raw `workspace_dir` if `canonicalize` fails (e.g.
    /// during early startup or in tests where the workspace doesn't exist on
    /// disk), matching the inline behavior the callers used before the cache.
    pub(super) async fn workspace_root(&self) -> PathBuf {
        self.canonical_workspace
            .get_or_init(|| async {
                tokio::fs::canonicalize(&self.workspace_dir)
                    .await
                    .unwrap_or_else(|_| self.workspace_dir.clone())
            })
            .await
            .clone()
    }

    /// Synchronous counterpart to [`workspace_root`], hydrating the **same**
    /// `canonical_workspace` cache via `OnceCell`'s sync `get`/`set`.
    ///
    /// The sync path validators (`is_path_string_allowed`,
    /// `is_resolved_path_allowed_for`) run on every file tool call and each
    /// previously re-invoked `self.workspace_dir.canonicalize()` — one
    /// `stat(2)` + symlink walk on the same immutable input per call. They
    /// cannot `.await` [`workspace_root`], so they reach the cache through this
    /// helper.
    ///
    /// # Why one cell can serve both, and why a lost `set` is safe
    ///
    /// This is the load-bearing claim, so it is stated here rather than left to
    /// a commit message. Sharing a cache between two producers is only sound if
    /// they cannot disagree, and here they *cannot*: `tokio::fs::canonicalize`
    /// is not a reimplementation, it is
    /// `asyncify(move || std::fs::canonicalize(path)).await` — literally the
    /// same `std` call, moved to a blocking thread. The async path and this one
    /// therefore run identical code over an input that is immutable for the
    /// life of a policy (`live_policy` rebuilds a fresh policy on reload, and
    /// `security_for_tool_context` overrides only `action_dir`/`trusted_roots`).
    ///
    /// The consequence is that the `get`/`set` race below needs no lock and no
    /// retry. If the async initializer wins between our `get` and our `set`,
    /// `set` fails and we return the value we just computed — which is the
    /// value the winner stored, because both are `std::fs::canonicalize` of the
    /// same path. A lost race costs one redundant `stat(2)` walk, never a
    /// divergent workspace root. Were that equivalence ever to break, this
    /// helper would need `get_or_init`, not a comment.
    ///
    /// Fallback to the raw `workspace_dir` on canonicalize failure matches the
    /// inline behavior these callers used before, and the async
    /// [`workspace_root`] — including under the race, since the fallback is the
    /// same immutable `workspace_dir` on both sides.
    pub(super) fn workspace_root_sync(&self) -> PathBuf {
        if let Some(cached) = self.canonical_workspace.get() {
            return cached.clone();
        }
        let canonical = self
            .workspace_dir
            .canonicalize()
            .unwrap_or_else(|_| self.workspace_dir.clone());
        // Deliberately ignoring the result: a failed `set` means the async
        // initializer won the race, and what it stored equals `canonical` —
        // see the equivalence argument above.
        let _ = self.canonical_workspace.set(canonical.clone());
        canonical
    }

    /// Validate a path for file I/O: string checks, canonicalize, workspace containment,
    /// and forbidden-path check on the resolved path.
    /// Returns the canonical `PathBuf` on success.
    pub async fn validate_path(&self, path: &str) -> Result<PathBuf, String> {
        if !self.is_path_string_allowed(path) {
            return Err(format!("{POLICY_BLOCKED_MARKER} Invalid path"));
        }

        // Ready media artifacts outside the ordinary roots are approved by the
        // artifact store before their exact path is added as a TinySecurity
        // reservation exception. TinySecurity still validates the final path.
        let expanded = self.expand_tilde(path);
        let candidate = if Path::new(&expanded).is_absolute() {
            PathBuf::from(&expanded)
        } else {
            self.action_dir.join(&expanded)
        };
        if let Ok(resolved) = tokio::fs::canonicalize(&candidate).await {
            if self.authorize_ready_media_artifact_path(&resolved).await {
                return self.validate_native_path(path, false).await;
            }
        }
        self.validate_native_path(path, false).await
    }

    /// Resolve an existing destination parent through TinySecurity's
    /// `ValidateParent`; the module owns policy, protected-floor and symlink
    /// decisions for the full prospective path.
    pub async fn validate_parent_path(&self, path: &str) -> Result<PathBuf, String> {
        if !self.is_path_string_allowed(path) {
            return Err(format!("{POLICY_BLOCKED_MARKER} Invalid path"));
        }
        self.validate_native_path(path, true).await
    }

    /// Returns `true` if `path` falls under one of the internal-state
    /// subdirectories or files within `workspace_dir`. Agent tools must not
    /// write to these locations — they contain memory DBs, session transcripts,
    /// tokens, and other core persistence that is not part of the agent's
    /// action surface.
    pub fn is_workspace_internal_path(&self, path: &Path) -> bool {
        // Try canonical forms first (handles symlinks), fall back to raw paths
        // when they don't exist on disk yet.
        let ws_canonical = self.workspace_dir.canonicalize();
        let path_canonical = path.canonicalize();
        let (ws, check_path) = match (&ws_canonical, &path_canonical) {
            (Ok(w), Ok(p)) => (w.as_path(), p.as_path()),
            _ => (self.workspace_dir.as_path(), path),
        };
        // The account config (`<openhuman_dir>/config.toml`, the workspace's
        // sibling) holds the autonomy policy itself and the files folders the
        // artifact escape guard trusts (`files_dir_override`,
        // `files_dir_history`, #5505). A trusted root over the account or data
        // dir must not let the agent rewrite either.
        if let Some(account_dir) = ws.parent() {
            if check_path == account_dir.join(ACCOUNT_CONFIG_FILE) {
                return true;
            }
        }
        if !check_path.starts_with(ws) {
            return false;
        }
        let relative = match check_path.strip_prefix(ws) {
            Ok(r) => r,
            Err(_) => return false,
        };
        let first_component = match relative.components().next() {
            Some(std::path::Component::Normal(s)) => s.to_string_lossy(),
            _ => return false,
        };
        let component = first_component.as_ref();
        if WORKSPACE_INTERNAL_DIRS.contains(&component)
            || ["memory-", "memory_tree-", "session_raw-"]
                .iter()
                .any(|prefix| {
                    component
                        .strip_prefix(prefix)
                        .is_some_and(|s| !s.is_empty())
                })
        {
            return true;
        }
        // Artifact metadata (#5505): `artifacts/<id>/meta.json` names the file
        // an artifact owns and Download / `read_artifact_bytes` follow it, so
        // every per-artifact directory is internal state. `artifacts/tool-
        // results/` is not: the agent reads its own large tool outputs back
        // from there.
        if component == ARTIFACTS_DIR {
            if let Some(std::path::Component::Normal(second)) = relative.components().nth(1) {
                if second != ARTIFACT_TOOL_RESULTS_DIR {
                    return true;
                }
            }
        }
        // Check single-file entries (only if the relative path is exactly one component)
        if relative.components().count() == 1
            && WORKSPACE_INTERNAL_FILES
                .iter()
                .any(|f| *f == first_component.as_ref())
        {
            return true;
        }
        false
    }

    /// Paths that remain blocked even when a `trusted_root` grant would
    /// otherwise reach them — credential stores and core OS directories. A
    /// grant on a parent must never expose SSH/GPG/AWS/keychain secrets, nor
    /// open `/etc`, `C:\Windows`, `/System`, etc. Matching is **case-insensitive**
    /// (Windows/macOS filesystems are), so `.SSH` / `C:\WINDOWS` cannot slip
    /// through. Gray-area dirs (`/usr`, `/opt`, `/var`, `~/Library`) stay in the
    /// user-overridable `forbidden_paths` instead, so a grant can still reach
    /// e.g. `/usr/local/...`.
    pub(crate) fn is_always_forbidden(path: &Path) -> bool {
        // Normalize separators + case BEFORE splitting: a Windows backslash
        // path is a single component on POSIX (and vice-versa), so we segment
        // the normalized string rather than rely on `Path::components()`.
        let lc_path = path
            .to_string_lossy()
            .to_ascii_lowercase()
            .replace('\\', "/");
        let segments: Vec<&str> = lc_path.split('/').filter(|s| !s.is_empty()).collect();

        // (a) Credential stores — matched by path segment, location-independent
        // (catches e.g. `C:\Users\x\.ssh` and `~/Library/Keychains`).
        if segments.iter().any(|s| SENSITIVE_COMPONENTS.contains(s)) {
            return true;
        }
        // Single-file credential stores have no protected parent directory.
        if segments.iter().any(|s| SENSITIVE_FILES.contains(s)) {
            return true;
        }
        if segments
            .windows(2)
            .any(|w| SENSITIVE_PAIRS.iter().any(|pair| w == *pair))
        {
            return true;
        }
        // (b) Core OS directories — matched by absolute prefix. Unconditional,
        // unlike the user-overridable `forbidden_paths`.
        ALWAYS_FORBIDDEN_PREFIXES
            .iter()
            .any(|p| lc_path == *p || lc_path.starts_with(&format!("{p}/")))
    }

    /// True if `path` is within a configured trusted root. When `require_write`
    /// is set, only `ReadWrite` roots match. Never matches credential stores.
    pub fn is_within_trusted_root(&self, path: &Path, require_write: bool) -> bool {
        if Self::is_always_forbidden(path) {
            return false;
        }
        self.trusted_roots.iter().any(|root| {
            if require_write && root.access != TrustedAccess::ReadWrite {
                return false;
            }
            let root_path = PathBuf::from(self.expand_tilde(&root.path));
            // `Path::starts_with("")` is true for every path, and a relative
            // root resolves against the process CWD: only an absolute root
            // grants anything.
            if !root_path.is_absolute() {
                return false;
            }
            let canonical_root = root_path
                .canonicalize()
                .unwrap_or_else(|_| root_path.clone());
            path.starts_with(&root_path) || path.starts_with(&canonical_root)
        })
    }

    /// Validate that a resolved path is still inside the workspace.
    /// Call this AFTER joining `workspace_dir` + relative path and canonicalizing.
    pub fn is_resolved_path_allowed(&self, resolved: &Path) -> bool {
        self.is_resolved_path_allowed_for(resolved, false)
    }

    /// Operation-aware resolved-path check: allowed when under the workspace, or
    /// within a trusted root (write roots only when `require_write`). Prefers the
    /// canonical workspace root so `/a/../b` style config paths don't misfire.
    pub fn is_resolved_path_allowed_for(&self, resolved: &Path, require_write: bool) -> bool {
        if Self::is_always_forbidden(resolved) {
            return false;
        }
        if !self.enabled {
            return true;
        }
        let workspace_root = self.workspace_root_sync();
        resolved.starts_with(&workspace_root)
            || self.is_within_trusted_root(resolved, require_write)
    }

    /// Check `resolved` against every entry in `forbidden_paths`, resolving relative
    /// entries against `workspace_root`. Absolute entries whose prefix IS the workspace
    /// root are skipped — the workspace containment check already covers them.
    pub(super) fn check_resolved_against_forbidden(
        &self,
        resolved: &Path,
        workspace_root: &Path,
        ready_media_artifact: bool,
    ) -> Result<(), String> {
        // Credential stores are never reachable, even via a trusted-root grant.
        if Self::is_always_forbidden(resolved) {
            return Err(format!(
                "{POLICY_BLOCKED_MARKER} Resolved path is a protected credential store: {}",
                resolved.display()
            ));
        }
        if !self.enabled {
            return Ok(());
        }
        // Trusted roots may override user-configured forbidden paths, but never
        // the core-managed workspace-state boundary.
        if self.is_workspace_internal_path(resolved) {
            return Err(format!(
                "{POLICY_BLOCKED_MARKER} Resolved path is workspace-internal application state: {}",
                resolved.display()
            ));
        }
        if ready_media_artifact {
            return Ok(());
        }
        // A trusted-root grant takes precedence over forbidden_paths for its subtree.
        if self.is_within_trusted_root(resolved, false) {
            return Ok(());
        }
        for forbidden in &self.forbidden_paths {
            let forbidden_path = PathBuf::from(self.expand_tilde(forbidden));
            let forbidden_resolved = if forbidden_path.is_absolute() {
                if workspace_root.starts_with(&forbidden_path) {
                    continue;
                }
                forbidden_path
            } else {
                workspace_root.join(forbidden_path)
            };
            if resolved.starts_with(&forbidden_resolved) {
                return Err(format!(
                    "{POLICY_BLOCKED_MARKER} Resolved path is inside a forbidden directory: {}",
                    forbidden_resolved.display()
                ));
            }
        }
        Ok(())
    }

    fn is_media_artifact_path_cached(&self, path: &Path) -> bool {
        self.media_artifact_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(path)
    }

    /// Resolve a candidate outside the normal tool roots through the durable
    /// artifact store. Only Ready image/video files in configured Files roots
    /// are cached for the read-only file inspection tools.
    async fn authorize_ready_media_artifact_path(&self, path: &Path) -> bool {
        if Self::is_always_forbidden(path) || self.is_workspace_internal_path(path) {
            return false;
        }
        let Some(roots) = self
            .media_files_roots
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
        else {
            self.forget_media_artifact_path(path);
            return false;
        };
        if !roots.contains_path(path) {
            self.forget_media_artifact_path(path);
            return false;
        }

        let artifacts =
            match crate::artifacts::store::list_artifacts(&self.workspace_dir, 0, usize::MAX, None)
                .await
            {
                Ok((artifacts, _)) => artifacts,
                Err(error) => {
                    tracing::warn!(
                        target: "security",
                        %error,
                        "[security] could not resolve a media artifact read"
                    );
                    return false;
                }
            };

        for meta in artifacts.iter().filter(|meta| {
            matches!(
                &meta.kind,
                crate::artifacts::ArtifactKind::Image | crate::artifacts::ArtifactKind::Video
            ) && matches!(&meta.status, crate::artifacts::ArtifactStatus::Ready)
        }) {
            let Ok(ready_path) =
                crate::artifacts::resolve_ready_file_meta(&self.workspace_dir, &roots, meta).await
            else {
                continue;
            };
            let Ok(ready_path) = tokio::fs::canonicalize(ready_path).await else {
                continue;
            };
            if ready_path == path {
                self.media_artifact_paths
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(ready_path);
                return true;
            }
        }
        self.forget_media_artifact_path(path);
        false
    }

    fn forget_media_artifact_path(&self, path: &Path) {
        self.media_artifact_paths
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(path);
    }
}
