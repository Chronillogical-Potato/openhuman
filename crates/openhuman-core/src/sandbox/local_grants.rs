//! Host adapter for the local jail's filesystem grants.
//!
//! TinyBox owns grant discovery (`ResolveLocalGrants`): toolchain homes, git
//! config includes, path normalisation and the credential-ancestor check. The
//! host owns the policy context: TinySecurity supplies its protected-path floor
//! and credential roots, which OpenHuman re-applies to returned grants before
//! passing them to TinyBox.

use std::path::PathBuf;

use super::types::SandboxPolicy;
use crate::config::{Config, LocalJailConfig};

/// Paths the jail is granted beyond the workspace root.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct JailGrants {
    pub read_only: Vec<PathBuf>,
    pub read_write: Vec<PathBuf>,
}

/// The operator home a grant set may be built from. A tenant never reads the
/// operator's home, so SaaS callers get none and `home` is not consulted.
pub(crate) fn grant_home(saas: bool, home: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
    if saas {
        None
    } else {
        home()
    }
}

/// Add the TinyBox-resolved toolchain grants to a local policy.
///
/// Explicit mounts already on the policy are kept. Without the `modules`
/// feature there is no jail to grant for, so the policy is returned as is.
pub(crate) async fn with_resolved_grants(
    config: &Config,
    policy: &SandboxPolicy,
) -> anyhow::Result<SandboxPolicy> {
    let mut policy = policy.clone();
    #[cfg(feature = "modules")]
    {
        let home = grant_home(crate::core::runtime::is_saas(), dirs::home_dir);
        let grants = resolve(config, home.as_deref(), &config.runtime.local_jail).await?;
        tracing::debug!(
            read_only = grants.read_only.len(),
            read_write = grants.read_write.len(),
            "[sandbox:grants] resolved local jail grants"
        );
        merge(&mut policy.read_only_mounts, grants.read_only);
        merge(&mut policy.read_write_mounts, grants.read_write);
    }
    #[cfg(not(feature = "modules"))]
    let _ = config;
    Ok(policy)
}

#[cfg(feature = "modules")]
fn merge(into: &mut Vec<PathBuf>, extra: Vec<PathBuf>) {
    for path in extra {
        if !into.contains(&path) {
            into.push(path);
        }
    }
}

#[cfg(feature = "modules")]
async fn resolve(
    config: &Config,
    home: Option<&std::path::Path>,
    jail: &LocalJailConfig,
) -> anyhow::Result<JailGrants> {
    let request = grant_request(home, jail).await.map_err(|error| {
        anyhow::anyhow!("TinySecurity could not provide the local jail path floor: {error}")
    })?;
    let reply = crate::modules::tinybox::resolve_local_grants(config, request)
        .await
        .map_err(|error| anyhow::anyhow!("TinyBox could not resolve local jail grants: {error}"))?;
    Ok(admit_module_grants(reply, jail.allow_proc).await)
}

/// Build the `ResolveLocalGrants` request from TinySecurity's floor vocabulary.
#[cfg(feature = "modules")]
pub(crate) async fn grant_request(
    home: Option<&std::path::Path>,
    jail: &LocalJailConfig,
) -> Result<crate::modules::tinybox::contract::ResolveLocalGrantsRequest, String> {
    use crate::modules::tinybox::contract::{
        LocalGrantConfig, ProtectedPaths, ResolveLocalGrantsRequest,
    };
    let floor = crate::modules::security::gap::protected_floor().await?;
    let credential_roots =
        crate::modules::security::gap::credential_roots(home.map(std::path::Path::to_path_buf))
            .await?;
    Ok(ResolveLocalGrantsRequest {
        home: home.map(|h| h.display().to_string()),
        config: LocalGrantConfig {
            toolchain_homes: jail.toolchain_homes,
            extra_read_only: jail.extra_read_only.clone(),
            extra_read_write: jail.extra_read_write.clone(),
            allow_proc: jail.allow_proc,
        },
        protected: ProtectedPaths {
            components: floor.components.into_iter().chain(floor.files).collect(),
            component_pairs: floor.pairs,
            prefixes: floor.prefixes,
            credential_roots: credential_roots
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
        },
    })
}

/// Second gate: drop every returned path the host floor forbids, and every
/// path that contains one of the operator's credential stores (a grant of
/// `$HOME` or `/` reaches `~/.ssh`, and Landlock cannot carve it back out).
/// A path is judged as the OS resolves it: one with `.`/`..` segments or a
/// rootless one is refused outright, and a symlink is checked at its target.
/// `/proc` is the one sanctioned exception to the floor, and only when
/// `allow_proc` is set.
#[cfg(feature = "modules")]
pub(crate) async fn admit_module_grants(
    reply: crate::modules::tinybox::contract::LocalGrants,
    allow_proc: bool,
) -> JailGrants {
    let home = grant_home(crate::core::runtime::is_saas(), dirs::home_dir);
    admit_grants_under(reply, allow_proc, home.as_deref()).await
}

#[cfg(feature = "modules")]
async fn admit_grants_under(
    reply: crate::modules::tinybox::contract::LocalGrants,
    allow_proc: bool,
    home: Option<&std::path::Path>,
) -> JailGrants {
    use std::path::Component;
    let stores = match crate::modules::security::gap::credential_roots(home.map(PathBuf::from))
        .await
    {
        Ok(roots) => roots,
        Err(error) => {
            tracing::warn!(%error, "[sandbox:grants] TinySecurity credential roots unavailable; refusing module grants");
            return JailGrants::default();
        }
    };
    let mut candidates = Vec::new();
    let mut lexical_paths = Vec::new();
    let mut canonical_paths = Vec::new();
    for raw in reply.read_only.iter().chain(reply.read_write.iter()) {
        let path = PathBuf::from(raw);
        let lexical = path.has_root()
            && path.components().all(|c| {
                matches!(
                    c,
                    Component::Prefix(_) | Component::RootDir | Component::Normal(_)
                )
            });
        if !lexical {
            tracing::warn!(
                path = %path.display(),
                "[sandbox:grants] refused module grant: not a plain absolute path"
            );
            candidates.push(None);
            continue;
        }
        let resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
        candidates.push(Some((lexical_paths.len(), path.clone(), resolved.clone())));
        lexical_paths.push(path);
        canonical_paths.push(resolved);
    }
    let protected = match crate::modules::security::gap::protected_paths(
        &lexical_paths
            .iter()
            .chain(&canonical_paths)
            .cloned()
            .collect::<Vec<_>>(),
    )
    .await
    {
        Ok(answers) if answers.len() == lexical_paths.len() * 2 => answers,
        _ => return JailGrants::default(),
    };
    let admit = |candidate: Option<(usize, PathBuf, PathBuf)>| -> Option<PathBuf> {
        let (index, path, resolved) = candidate?;
        let sanctioned_proc = allow_proc && path == std::path::Path::new("/proc");
        if !sanctioned_proc && (protected[index] || protected[index + lexical_paths.len()]) {
            tracing::warn!(path = %path.display(), "[sandbox:grants] TinySecurity protected-path floor refused module grant");
            return None;
        }
        if stores.iter().any(|store| {
            store.starts_with(&path)
                || store.starts_with(&resolved)
                || path.starts_with(store)
                || resolved.starts_with(store)
        }) {
            tracing::warn!(path = %path.display(), "[sandbox:grants] refused module grant overlapping a credential root");
            return None;
        }
        Some(path)
    };
    let split = reply.read_only.len();
    JailGrants {
        read_only: candidates[..split]
            .iter()
            .cloned()
            .filter_map(admit)
            .collect(),
        read_write: candidates[split..]
            .iter()
            .cloned()
            .filter_map(admit)
            .collect(),
    }
}

#[cfg(test)]
#[path = "local_grants_tests.rs"]
mod tests;
