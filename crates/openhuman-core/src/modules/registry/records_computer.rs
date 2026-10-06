//! Native computer-use module (desktop and browser control). Published digests are added only from its release manifest.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};
use tinycomputer_bus::names;

pub(crate) const TINYCOMPUTER: ModuleRecord = ModuleRecord {
    id: "tinycomputer",
    description: "Permission-aware desktop and browser observation and control",
    bus_name: names::INTERFACE,
    object_path: names::OBJECT_PATH,
    version: "0.9.1",
    release_url: "https://github.com/tinyhumansai/tinycomputer/releases/tag/v0.9.1",
    // Verbatim from the published tinycomputer v0.9.0 checksum.toml. Linux remains outside
    // the initial product surface; this registry admits macOS and Windows.
    assets: &[
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinycomputer-0.9.1-macos-26-arm64.tar.gz",
            sha256: "11ce9bf65f2ea4545e82ebb149118a5cd53bd8d48dc896778d11b01096f044b9",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinycomputer-0.9.1-macos-26-x86_64.tar.gz",
            sha256: "62af5dedf31ebcd6e6c5d1922159fd13d70c3a898e1eab56c0a5146d5640f0bf",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinycomputer-0.9.1-macos-15-arm64.tar.gz",
            sha256: "3bd4dbe13633c4f01d057dd6ebb25626c0b38272ad7f2242f6af5cd3b34c2dc9",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinycomputer-0.9.1-macos-15-x86_64.tar.gz",
            sha256: "70ead83a12b6e5283de2a49766199e61f4f15f8820068d61905f7571a489f8a3",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinycomputer-0.9.1-windows-2025-x86_64.zip",
            sha256: "c8b7c117efb67d7450adcbe2ae81f3de8bcdc1e9b583a7f996e51b61e1e777cf",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinycomputer-0.9.1-windows-2022-x86_64.zip",
            sha256: "3c3779198ca9a9a84dba8e8a5bee6e12da4a395ea7c330303d542c8c194ba280",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinycomputer-0.9.1-windows-11-arm64.zip",
            sha256: "e1e8c8e521e186c3321e972d24f610ecf272151cc3b9c1aeaaccf3fc4cc15ad0",
        },
    ],
    load: LoadPolicy::Lazy,
};
