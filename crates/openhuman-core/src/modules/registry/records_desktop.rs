//! Native desktop module. Published digests are added only from its release manifest.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};
use tinydesktop_bus::names;

pub(crate) const TINYDESKTOP: ModuleRecord = ModuleRecord {
    id: "tinydesktop",
    description: "Permission-aware native desktop observation and control",
    bus_name: names::INTERFACE,
    object_path: names::OBJECT_PATH,
    version: "0.3.0",
    release_url: "https://github.com/tinyhumansai/tinydesktop/releases/tag/v0.3.0",
    // Verbatim from the published v0.3.0 checksum.toml. Linux remains outside
    // the initial product surface; this registry admits macOS and Windows.
    assets: &[
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinydesktop-0.3.0-macos-26-arm64.tar.gz",
            sha256: "83d245497c15ee7a385d53cbd0f9b251b7ddc90c76b6bc2ec96b77b76cc909b7",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinydesktop-0.3.0-macos-26-x86_64.tar.gz",
            sha256: "4aec4734420246fe1f67cce49eeb29975b03451bc6616cd4cac8171f4d0283d6",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinydesktop-0.3.0-macos-15-arm64.tar.gz",
            sha256: "fc9449083fe4030c77f910d5ebb609b881e4c51429689b710ae3623dd67e2b03",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinydesktop-0.3.0-macos-15-x86_64.tar.gz",
            sha256: "d319d8a491496f3da8d7f2df58c08665e61f09e28474f6d88ccc38a1cf37bd38",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinydesktop-0.3.0-windows-2025-x86_64.zip",
            sha256: "bbb48068c87fb1321bb3b8eefc652fc1d40045b80b9357fb43cbc0a4e9833334",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinydesktop-0.3.0-windows-2022-x86_64.zip",
            sha256: "11975cb1d964665af3e1b3baada61f552ca3c55e2e1b4e550ecb39443fad9f9f",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinydesktop-0.3.0-windows-11-arm64.zip",
            sha256: "ada3f5f05d7bc7a00c5df4cb671f1988e3b86b8bf0ebfe9fa6ae2af2092c6f64",
        },
    ],
    load: LoadPolicy::Lazy,
};
