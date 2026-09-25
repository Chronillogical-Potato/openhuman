//! Native desktop module. Published digests are added only from its release manifest.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};
use tinydesktop_bus::names;

pub(crate) const TINYDESKTOP: ModuleRecord = ModuleRecord {
    id: "tinydesktop",
    description: "Permission-aware native desktop observation and control",
    bus_name: names::INTERFACE,
    object_path: names::OBJECT_PATH,
    version: "0.4.0",
    release_url: "https://github.com/tinyhumansai/tinydesktop/releases/tag/v0.4.0",
    // Verbatim from the published v0.4.0 checksum.toml. Linux remains outside
    // the initial product surface; this registry admits macOS and Windows.
    assets: &[
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinydesktop-0.4.0-macos-26-arm64.tar.gz",
            sha256: "85a5a43e5b09d05d05dbb9e85e9bc5fa0dc2b3edcbf230fe491c7270aab96b37",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinydesktop-0.4.0-macos-26-x86_64.tar.gz",
            sha256: "70328d21006c6f1b32a4a3b0db872bbffe93bb7b3eee7d22cd0289c596a2f826",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinydesktop-0.4.0-macos-15-arm64.tar.gz",
            sha256: "77788b093768da5bf569148d2f90faea0888590827ce769403197b4fe42fc542",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinydesktop-0.4.0-macos-15-x86_64.tar.gz",
            sha256: "00fbdda30538e802e01385fd1cdfd926a3b4e202940108d746ac447f46aa3e6a",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinydesktop-0.4.0-windows-2025-x86_64.zip",
            sha256: "77aad590c206e63667d581b4fcd9fee494ab61524c64921b731ba345ccf55588",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinydesktop-0.4.0-windows-2022-x86_64.zip",
            sha256: "dc576abc142b50606f9d2eab1a72d3be9c96b860e3315022ca415175734671ff",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinydesktop-0.4.0-windows-11-arm64.zip",
            sha256: "3f497b04d8472c11c08788f78ef49c94abaee1fbb91a0ac44ffb5dc708dd4098",
        },
    ],
    load: LoadPolicy::Lazy,
};
