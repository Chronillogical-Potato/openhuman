//! Storage backend configuration (`[storage]`).
//!
//! Where the core keeps durable state that has moved onto the
//! `tinystoragedrivers` ports. Empty (the default) keeps the classic on-disk
//! layout under the workspace, which is what the desktop app uses. A URL
//! points the process at one backend instead:
//!
//! ```toml
//! [storage]
//! url = "mongodb://app:secret@db.internal/openhuman"   # cloud, multi-tenant
//! # url = "sqlite:/var/lib/openhuman/storage"          # one SQLite dir
//! # url = "memory"                                    # tests, keeps nothing
//! ```
//!
//! `OPENHUMAN_STORAGE_URL` overrides it (see [`crate::storage`]). This
//! section is bootstrap configuration: it is always read from the file or the
//! environment, never from storage itself.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(default)]
pub struct StorageConfig {
    /// The storage URL: `memory`, `sqlite:<path>`, `mongodb://…/<db>`,
    /// `mongodb+srv://…/<db>` or `file:<dir>`. Empty or absent keeps the
    /// classic on-disk layout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl std::fmt::Debug for StorageConfig {
    /// Redacts credentials: a MongoDB URL carries the database password.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StorageConfig")
            .field("url", &self.url.as_deref().map(redact_url))
            .finish()
    }
}

/// `url` with any `user:password@` userinfo replaced by `***@`.
pub fn redact_url(url: &str) -> String {
    match (url.find("://"), url.rfind('@')) {
        (Some(scheme), Some(at)) if at > scheme => {
            format!("{}://***@{}", &url[..scheme], &url[at + 1..])
        }
        _ => url.to_string(),
    }
}

#[cfg(test)]
#[path = "storage_tests.rs"]
mod tests;
