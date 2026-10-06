//! Registry record for the `tinyjuice` module.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

pub(crate) const TINYJUICE: ModuleRecord = ModuleRecord {
    id: "tinyjuice",
    description: "Content-aware tool-output compression and recoverable caching",
    bus_name: "ai.tinyhumans.tinyjuice.Compression",
    object_path: "/ai/tinyhumans/tinyjuice/Compression",
    version: "0.6.0",
    release_url: "https://github.com/tinyhumansai/tinyjuice/releases/tag/v0.6.0",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyjuice-module-0.6.0-ubuntu-24.04-x86_64.tar.gz",
            sha256: "91ebe1a672d326bfdc92c195f7e5db6189e6e9e10e4f4fe0852219c12a86ae17",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyjuice-module-0.6.0-ubuntu-24.04-arm64.tar.gz",
            sha256: "78c80c6267eeb85e6f1f37e0c6d574b4362ad071745dc97477f6a44e81a14217",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyjuice-module-0.6.0-ubuntu-22.04-x86_64.tar.gz",
            sha256: "25f7c851a7f2dc5d746ee32a632863b783d76deee6ab0d786ea406238a33f27d",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyjuice-module-0.6.0-ubuntu-22.04-arm64.tar.gz",
            sha256: "29accacc64e87ce02ab8d56319149e2e91bf7a40965dbabbb537f3b648b77207",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyjuice-module-0.6.0-macos-26-arm64.tar.gz",
            sha256: "d37dc1f5cc370e6c4d1dc3580bc90bc055c18448e4de08f2149085aa721120c6",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyjuice-module-0.6.0-macos-26-x86_64.tar.gz",
            sha256: "3e5a08d16d8bdbb74e1ddb51adee53fc78b667ba68396625bc7de322239b9a83",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyjuice-module-0.6.0-macos-15-arm64.tar.gz",
            sha256: "00d67a7031b4a89c59950da2bfc1461b99b44247df9a026231ffef1af1ba299d",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyjuice-module-0.6.0-macos-15-x86_64.tar.gz",
            sha256: "077d8a501f915bb0ddab96b5cf21d202bf849b343dccab9b4b3f287fbaf0c50a",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyjuice-module-0.6.0-windows-2025-x86_64.zip",
            sha256: "a390bca33c1e8de560277f8041ac51bb317bc066f901424a8047a079e389f587",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyjuice-module-0.6.0-windows-2022-x86_64.zip",
            sha256: "4e6d3f621ed98a4fb8b689fe8a84c34e84cadb4a93e18e2b49d23daf92fc408c",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyjuice-module-0.6.0-windows-11-arm64.zip",
            sha256: "9ca0662f9eaa17ef8b0ab509b747efc3e9bcbfe2242dc12475d41a5b97364a58",
        },
    ],
    load: LoadPolicy::Lazy,
};
