//! Native computer-use module (desktop and browser control). Published digests are added only from its release manifest.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};
use tinycomputer_bus::names;

pub(crate) const TINYCOMPUTER: ModuleRecord = ModuleRecord {
    id: "tinycomputer",
    description: "Permission-aware desktop and browser observation and control",
    bus_name: names::INTERFACE,
    object_path: names::OBJECT_PATH,
    version: "0.10.0",
    release_url: "https://github.com/tinyhumansai/tinycomputer/releases/tag/v0.10.0",
    // Verbatim from the published tinycomputer v0.10.0 checksum.toml. Linux remains outside
    // the initial product surface; this registry admits macOS and Windows.
    assets: &[
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinycomputer-0.10.0-macos-26-arm64.tar.gz",
            sha256: "3f774d47841c9da0d4603072baa70089dba675f54a6fff9213874737e034ae9b",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinycomputer-0.10.0-macos-26-x86_64.tar.gz",
            sha256: "2e749aac2ad87037350c9611b3e8159c75b0d4825ce9d2624b8d9bbf1eddef4d",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinycomputer-0.10.0-macos-15-arm64.tar.gz",
            sha256: "01cbf581eba91a8c3de836b76e308afe41d0adc4b72b6960df15b9f8d02f70df",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinycomputer-0.10.0-macos-15-x86_64.tar.gz",
            sha256: "38e3ca6d8dfd42a093ae6dadd1527701c50a9f034ee5123df0011f982d87277f",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinycomputer-0.10.0-windows-2025-x86_64.zip",
            sha256: "e20c78b71801f0936dccf2fb0bc483b2541dc2de3fa9c46719e3b4d5fe3d2637",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinycomputer-0.10.0-windows-2022-x86_64.zip",
            sha256: "5c30932180cd7b9f78bb1f5dce1c0e6b1598755047fb3f4b57994167c4b787eb",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinycomputer-0.10.0-windows-11-arm64.zip",
            sha256: "61b4aa61fe659ad2b1b3692512cdbc9cee81cbbb28510bbced3998dd81c8f42b",
        },
    ],
    load: LoadPolicy::Lazy,
};
