//! Registry record for the `tinyjuice` module.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset, SourceSpec};

pub(crate) const TINYJUICE: ModuleRecord = ModuleRecord {
    id: "tinyjuice",
    description: "Content-aware tool-output compression and recoverable caching",
    bus_name: tinyjuice_bus::names::BUS_NAME,
    object_path: tinyjuice_bus::names::OBJECT_PATH,
    version: "0.11.4",
    release_url: "https://github.com/tinyhumansai/tinyjuice/releases/tag/v0.11.4",
    assets: &[
        PlatformAsset {
            host_key: "archlinux-arm64",
            archive: "tinyjuice-module-0.11.4-archlinux-arm64.tar.gz",
            sha256: "913911c13c68a03c6880f4a2f93a86ae9b5b90e66b54a7d56afcca4b891abc1c",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "archlinux-x86_64",
            archive: "tinyjuice-module-0.11.4-archlinux-x86_64.tar.gz",
            sha256: "7bbb73168f5a3e1dcb1dee5d293f2c89051385fe91fd7511d215703fddeb0d6c",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "fedora-43-arm64",
            archive: "tinyjuice-module-0.11.4-fedora-43-arm64.tar.gz",
            sha256: "d88fec0b1d66947f4ec50ed4a885c70931b1d30a6334c908e21b93546859eb4f",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "fedora-43-x86_64",
            archive: "tinyjuice-module-0.11.4-fedora-43-x86_64.tar.gz",
            sha256: "38771e23681b89d1799a5e9d8d561f474658b88451962cc404862f9cf57c7ac4",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "fedora-44-arm64",
            archive: "tinyjuice-module-0.11.4-fedora-44-arm64.tar.gz",
            sha256: "71de786cd9f1dd7a63c2336be851f964cca503ef4574002e118b6ecbf923d4b3",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "fedora-44-x86_64",
            archive: "tinyjuice-module-0.11.4-fedora-44-x86_64.tar.gz",
            sha256: "b193f60f4a57a3206fdec635c29dea35a09dfd4775c4feaff665e2e2ce9533c0",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyjuice-module-0.11.4-macos-15-arm64.tar.gz",
            sha256: "4203b9f0bc65ff6b9fba0d0c2c8cdf520394667bac376ad73d04c34df44800e0",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyjuice-module-0.11.4-macos-15-x86_64.tar.gz",
            sha256: "0e15dd99995f507545185cc39b54935b5256aabf6399a3bbfd6cede6c55a8328",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyjuice-module-0.11.4-macos-26-arm64.tar.gz",
            sha256: "bcc62ea985d69472970b3610e6f4906818acc6989185ed4db19c283a5f10553b",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyjuice-module-0.11.4-macos-26-x86_64.tar.gz",
            sha256: "fa423c86b140b3a36b617ee6a9f8a41be3a114b86f92454e3088c59c1fe7724c",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyjuice-module-0.11.4-ubuntu-22.04-arm64.tar.gz",
            sha256: "011571c705eb2249b702236a404fe29f9aceac84f9e6a2a156225bdc11165c5c",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyjuice-module-0.11.4-ubuntu-22.04-x86_64.tar.gz",
            sha256: "efb026415dd53291edd10062374b01c1cc90b64bfcada46f5fe55544686d6c7e",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyjuice-module-0.11.4-ubuntu-24.04-arm64.tar.gz",
            sha256: "b6cc845765722f23efe15973d0f10a57024ecf5c42e1c04c841240e633d3810d",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyjuice-module-0.11.4-ubuntu-24.04-x86_64.tar.gz",
            sha256: "3d46bc6b8494edf0e2daae0fa3c87313826c44d1a203ec532c2d7af0913cb1eb",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyjuice-module-0.11.4-windows-11-arm64.zip",
            sha256: "7399235fa05d3f7dfb71d1f3d1a15e1a0918ebfdde912f0030ae238a2e56a20f",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyjuice-module-0.11.4-windows-2022-x86_64.zip",
            sha256: "c11bc6fd61f2b4a5673a869e7075c777f0fb29d31e642dd9cd132d5849f3bb46",
            library_sha256: None,
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyjuice-module-0.11.4-windows-2025-x86_64.zip",
            sha256: "c58dd3af37c898afde8ec736452dc18e64c2d0269a480fa1e95bcce61ee7e944",
            library_sha256: None,
        },
    ],
    load: LoadPolicy::Lazy,
    source: SourceSpec {
        submodule: "tinyjuice",
        manifest: "crates/tinyjuice-module/Cargo.toml",
        package: "tinyjuice-module",
        lib_name: "tinyjuice_module",
        features: &[],
    },
};
