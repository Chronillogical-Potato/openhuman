//! Native desktop module. Published digests are added only from its release manifest.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};
use tinydesktop_bus::names;

pub(crate) const TINYDESKTOP: ModuleRecord = ModuleRecord {
    id: "tinydesktop",
    description: "Permission-aware native desktop observation and control",
    bus_name: names::INTERFACE,
    object_path: names::OBJECT_PATH,
    version: "0.5.1",
    release_url: "https://github.com/tinyhumansai/tinydesktop/releases/tag/v0.5.1",
    // Verbatim from the published v0.5.1 checksum.toml. Linux remains outside
    // the initial product surface; this registry admits macOS and Windows.
    assets: &[
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinydesktop-0.5.1-macos-26-arm64.tar.gz",
            sha256: "41b32370ec8789891f42bbcb40d7e1ea8c359ce1edbe210d1cd636ba2ba2562b",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinydesktop-0.5.1-macos-26-x86_64.tar.gz",
            sha256: "6961eb4b14013dfca6debb892f69fc995b92a99af30f61c37960dfe5a2e4c628",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinydesktop-0.5.1-macos-15-arm64.tar.gz",
            sha256: "b62d1d76d41649756e95ae536679cfd48250fadfe9549e973f2511183e7c9759",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinydesktop-0.5.1-macos-15-x86_64.tar.gz",
            sha256: "b0736c70706f7ea656e2098b3e055c6588d14811c83c82718be44fb78624e861",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinydesktop-0.5.1-windows-2025-x86_64.zip",
            sha256: "0216a29611498e95e812b0345269d887df8aae842bb31d43e7342ac3a8aa1e3e",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinydesktop-0.5.1-windows-2022-x86_64.zip",
            sha256: "4843d60778b7c8ddb58aa61c247b15ee8f8290f1a1535e46447623cee1855dfe",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinydesktop-0.5.1-windows-11-arm64.zip",
            sha256: "e5bf43a82a5950c46aa365210775fe4645b33c1e30573425b5b6797185466e74",
        },
    ],
    load: LoadPolicy::Lazy,
};
