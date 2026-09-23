//! Registry records for the `tinymemory` and `tinyjuice` modules.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The complete TinyMemory engine, loaded eagerly so its capabilities are
/// available when the kernel assembles its RPC and tool surfaces.
pub(crate) const TINYMEMORY: ModuleRecord = ModuleRecord {
    id: "tinymemory",
    description: "Local memory engine: store, ranked recall, and portable export",
    bus_name: "ai.tinyhumans.tinymemory.Memory",
    object_path: "/ai/tinyhumans/tinymemory/Memory",
    version: "1.16.0",
    release_url: "https://github.com/tinyhumansai/tinymemory/releases/tag/v1.16.0",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinymemory-module-1.16.0-ubuntu-24.04-x86_64.tar.gz",
            sha256: "fc65fce075b0d286b0d1cce48fc8952c2936e4b945f2af84b6a20d800c2a10d7",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinymemory-module-1.16.0-ubuntu-24.04-arm64.tar.gz",
            sha256: "1735efb7b0b6a56c85da1b62fbfa2d2a68995a04203ec2d79b4e1b389935fd92",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinymemory-module-1.16.0-ubuntu-22.04-x86_64.tar.gz",
            sha256: "f3ba06867ec89b8374a405f8a8569f5cecf88490609354ae4a6c1faa6e55b425",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinymemory-module-1.16.0-ubuntu-22.04-arm64.tar.gz",
            sha256: "b9f6794806e9463cffbfc3ce0c4b4b39cb8c1f3403a2ba956b0984cc35e9354a",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinymemory-module-1.16.0-macos-26-arm64.tar.gz",
            sha256: "097ab5fdc352f84f34b770302db73c5c1468e7ad48709e1767e74d61cc55646c",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinymemory-module-1.16.0-macos-26-x86_64.tar.gz",
            sha256: "3128eaff5e820c86fabfbe6a759976150313ca3c2c38e00bb2bb67dd2dfa76ba",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinymemory-module-1.16.0-macos-15-arm64.tar.gz",
            sha256: "fa9d65f31b7ece3eed0d118795ce06b66f0bd271556b91ccad09b0c4b41d94f9",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinymemory-module-1.16.0-macos-15-x86_64.tar.gz",
            sha256: "ac1343f128cdd4b299b43318019d1b26c0c8fc28abf62bf24ff8fb6d9894730f",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinymemory-module-1.16.0-windows-2025-x86_64.zip",
            sha256: "dcf5ec88583a02b284f0b430a037230a3d974703d39f11be53a4e36688c8d7db",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinymemory-module-1.16.0-windows-2022-x86_64.zip",
            sha256: "cf56453fba522a075e569d60c39d7e56e938ff2d4fbb8270debaffd7e6b39819",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinymemory-module-1.16.0-windows-11-arm64.zip",
            sha256: "9d0ca43cc3c7cac29e631e9870fc30d3fd4971e272e393ed2a91adb7ca20fdf3",
        },
    ],
    // Eager, unlike the two codecs above. A codec that is never asked for should
    // not be paid for, but a memory driver's absence changes what the kernel
    // offers rather than merely delaying it: capabilities are read at bind time
    // and the RPC surface and agent-tool list are filtered from them. Resolving
    // that during a user's first recall would mean the first recall is the one
    // that behaves differently.
    load: LoadPolicy::Eager,
};

/// The `tinyjuice` content-aware tool-output compression engine.
///
/// Lazy because the host's compaction policy can disable it, and a session that
/// never produces compressible tool output should not pay the download or
/// resident native-library cost.
pub(crate) const TINYJUICE: ModuleRecord = ModuleRecord {
    id: "tinyjuice",
    description: "Content-aware tool-output compression and recoverable caching",
    bus_name: "ai.tinyhumans.tinyjuice.Compression",
    object_path: "/ai/tinyhumans/tinyjuice/Compression",
    version: "0.3.0",
    release_url: "https://github.com/tinyhumansai/tinyjuice/releases/tag/v0.3.0",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyjuice-module-0.3.0-ubuntu-24.04-x86_64.tar.gz",
            sha256: "e1cd72b10ba2394fae91569b452efd87e5e6ec7e11d2747122283b3d031bf9ea",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyjuice-module-0.3.0-ubuntu-24.04-arm64.tar.gz",
            sha256: "d134b1973cf5b88666eb7b9a712c5b8ce1fb34999ae45112003ecb4180f56877",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyjuice-module-0.3.0-ubuntu-22.04-x86_64.tar.gz",
            sha256: "7ad0d7741d47bfa6d1e6387c42bbad9b75fec152c7b9cb8ef5dc0d00a5457ac1",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyjuice-module-0.3.0-ubuntu-22.04-arm64.tar.gz",
            sha256: "a09341688b8a3cc3b1402968e9f151e3e67bd8a82afbd5de0c4d4e962ea54903",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyjuice-module-0.3.0-macos-26-arm64.tar.gz",
            sha256: "96a8e3a9ff339650ad34e9c1b34c8441416853f0cab8c07c13871e1e427bc9a5",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyjuice-module-0.3.0-macos-26-x86_64.tar.gz",
            sha256: "f47384306efba93ecdf6f3876b73ad78d221c23bd89537e4c16aff33c593c2ad",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyjuice-module-0.3.0-macos-15-arm64.tar.gz",
            sha256: "4874d731fdd6e1d8799db7aaeaeddc9ec2151011ac5615d3a5bd0b17be7c45d0",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyjuice-module-0.3.0-macos-15-x86_64.tar.gz",
            sha256: "7855e626c1ab6d348a3c3e4110b2ec02bf3ac7e5e7b7e8d02793e5d2fcee00fa",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyjuice-module-0.3.0-windows-2025-x86_64.zip",
            sha256: "daca832e3e8e1e14bb759e8940ef7368c4821958ac159434b4be100cab37bbb1",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyjuice-module-0.3.0-windows-2022-x86_64.zip",
            sha256: "c3344fd8123a080d778bb7881dd6b3d4d5449a8845ea4bd6ba21fb34eeb73812",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyjuice-module-0.3.0-windows-11-arm64.zip",
            sha256: "dadd62df0442363a81f6aa2ccb622c58b0b18b2b6717f539c2603e3109065c4b",
        },
    ],
    load: LoadPolicy::Lazy,
};
