//! Site files: where each site's memory is kept, reading one back, writing a
//! change through a temporary file within its size limit, and removing what
//! is forgotten or unchanged for 30 days.

use std::collections::BTreeSet;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use tokio::io::AsyncWriteExt;

use super::{now, SiteMemory, KEEP_SECS, MAX_FILE_BYTES};
use crate::config::Config;

/// Removes every file in `dir`, returning how many sites they held.
pub(super) async fn forget_every_site(dir: &Path) -> std::io::Result<usize> {
    let mut entries = match tokio::fs::read_dir(dir).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };
    let mut forgotten = BTreeSet::new();
    while let Some(entry) = entries.next_entry().await? {
        if entry.file_type().await?.is_file() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some(site) = name
                .strip_suffix(".json")
                .or_else(|| name.strip_suffix(".json.corrupt"))
            {
                forgotten.insert(site.to_owned());
            }
            remove(&entry.path()).await?;
        }
    }
    Ok(forgotten.len())
}

/// Removes the file at `path`, returning whether there was one.
pub(super) async fn remove(path: &Path) -> std::io::Result<bool> {
    match tokio::fs::remove_file(path).await {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

/// Removes the site files nothing changed for `KEEP_SECS` before `now`:
/// every entry in them has expired, as has a copy set aside or a write left
/// behind.
pub(super) async fn sweep(config: &Config, now: u64) {
    let _saving = saving().lock().await;
    let Ok(mut entries) = tokio::fs::read_dir(sites_dir(config)).await else {
        return;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let Ok(metadata) = entry.metadata().await else {
            continue;
        };
        let changed = metadata
            .modified()
            .ok()
            .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
            .map_or(now, |since| since.as_secs());
        if metadata.is_file() && changed.saturating_add(KEEP_SECS) <= now {
            match remove(&entry.path()).await {
                Ok(_) => {
                    tracing::debug!("[browser-sites] removed a site file unchanged for 30 days")
                }
                Err(error) => tracing::warn!(%error, "[browser-sites] old site file not removed"),
            }
        }
    }
}

pub(super) fn sites_dir(config: &Config) -> PathBuf {
    crate::modules::computer_config::trace_dir(config).join("sites")
}

pub(super) fn site_path(config: &Config, site: &str) -> PathBuf {
    sites_dir(config).join(format!("{site}.json"))
}

/// `site`'s memory as of `now`. A missing file is an empty memory; a file
/// that cannot be read as one is set aside as `<site>.json.corrupt` and
/// treated as empty.
pub(super) async fn load(config: &Config, site: &str, now: u64) -> SiteMemory {
    let path = site_path(config, site);
    let read = match tokio::fs::metadata(&path).await {
        Ok(metadata) if metadata.len() <= MAX_FILE_BYTES => tokio::fs::read(&path).await,
        Ok(_) => Err(std::io::Error::other("larger than a site file can be")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return SiteMemory::default();
        }
        Err(error) => Err(error),
    };
    let parsed = read.and_then(|bytes| {
        serde_json::from_slice::<SiteMemory>(&bytes).map_err(std::io::Error::other)
    });
    match parsed {
        Ok(mut memory) => {
            memory.expire(now);
            memory
        }
        Err(error) => {
            tracing::warn!(%error, "[browser-sites] unreadable site file set aside");
            let _set_aside = tokio::fs::rename(&path, path.with_extension("json.corrupt")).await;
            SiteMemory::default()
        }
    }
}

/// Loads `site`'s memory and lets `change` change it, one update at a time,
/// writing it back when `change` says it did; a memory left empty removes
/// the file. Whether the file holds the change, or there was none; a
/// failure is logged, never raised.
pub(super) async fn update(
    config: &Config,
    site: &str,
    change: impl FnOnce(&mut SiteMemory) -> bool,
) -> bool {
    let _saving = saving().lock().await;
    let mut memory = load(config, site, now()).await;
    if !change(&mut memory) {
        return true;
    }
    let path = site_path(config, site);
    let written = if memory.plans.is_empty() && memory.hints.is_empty() {
        remove(&path).await.map(|_removed| ())
    } else {
        match fitted(&mut memory) {
            Ok(bytes) => save(&path, &bytes).await,
            Err(error) => Err(error),
        }
    };
    if let Err(error) = &written {
        tracing::warn!(%error, "[browser-sites] site memory not saved");
    }
    written.is_ok()
}

/// `memory` serialized within `MAX_FILE_BYTES`, its oldest plans and then its
/// oldest elements dropped until it fits: a larger file is set aside unread
/// by the next [`load`], with all it held.
pub(super) fn fitted(memory: &mut SiteMemory) -> std::io::Result<Vec<u8>> {
    loop {
        let bytes = serde_json::to_vec_pretty(memory).map_err(std::io::Error::other)?;
        let fits = u64::try_from(bytes.len()).is_ok_and(|length| length <= MAX_FILE_BYTES);
        if fits {
            return Ok(bytes);
        }
        if memory.plans.is_empty() {
            if memory.hints.is_empty() {
                return Ok(bytes);
            }
            memory.hints.remove(0);
        } else {
            memory.plans.remove(0);
        }
    }
}

/// Writes `bytes` to `path` through a temporary file, in a directory and a
/// file only their owner can read: plans and elements tell where a person
/// browses.
async fn save(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        tokio::fs::create_dir_all(dir).await?;
        #[cfg(unix)]
        tokio::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).await?;
    }
    let staged = path.with_extension("json.tmp");
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&staged).await?;
    #[cfg(unix)]
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .await?;
    file.write_all(bytes).await?;
    file.flush().await?;
    drop(file);
    tokio::fs::rename(&staged, path).await
}

/// Serializes changes to site files across tasks.
pub(super) fn saving() -> &'static tokio::sync::Mutex<()> {
    static SAVING: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    SAVING.get_or_init(|| tokio::sync::Mutex::new(()))
}
