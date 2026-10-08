//! Owner-only writes of the local memory state files.
//!
//! The files under `<workspace>/memory/` (the job queue, channel threads,
//! source sync state, the layout migration state, the local install identity) hold ids and source
//! settings that are the person's own. `std::fs::write` creates them with
//! the process umask (0644 under the usual 022), readable by every local
//! account; here, on unix, a file is created (and kept) 0600 and its
//! directory 0700, as the config and keyring files already are. Other
//! platforms keep their default ACLs.

use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Owner read/write only.
#[cfg(unix)]
pub const FILE_MODE: u32 = 0o600;

/// Owner read/write/search only.
#[cfg(unix)]
pub const DIR_MODE: u32 = 0o700;

/// Creates `dir` and any missing parent, each new one 0700 on unix, and
/// narrows `dir` itself to 0700 when it already existed.
///
/// # Errors
///
/// When a directory cannot be created or its mode set.
pub fn create_private_dir_all(dir: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(DIR_MODE)
            .create(dir)?;
        let mode = std::fs::metadata(dir)?.permissions().mode() & 0o777;
        if mode != DIR_MODE {
            tracing::debug!(
                dir = %dir.display(),
                from = format!("{mode:o}"),
                "[memory:files] narrowing a state directory to 0700"
            );
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(DIR_MODE))?;
        }
        Ok(())
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(dir)
    }
}

/// Creates (or truncates) `path` for writing, 0600 on unix whether it is
/// new or already existed with a wider mode.
///
/// # Errors
///
/// When the file cannot be opened or its mode set.
pub fn create_private(path: &Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(FILE_MODE);
        let file = options.open(path)?;
        // `mode` applies only to a file this call creates.
        file.set_permissions(std::fs::Permissions::from_mode(FILE_MODE))?;
        Ok(file)
    }
    #[cfg(not(unix))]
    {
        options.open(path)
    }
}

/// Writes `bytes` to `path` owner-only ([`create_private`]), creating its
/// directory owner-only ([`create_private_dir_all`]).
///
/// # Errors
///
/// When the directory or file cannot be created or written.
pub fn write_private(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        create_private_dir_all(dir)?;
    }
    create_private(path)?.write_all(bytes)
}

#[cfg(test)]
#[path = "files_tests.rs"]
mod tests;
