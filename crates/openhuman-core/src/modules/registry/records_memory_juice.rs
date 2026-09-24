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
    version: "0.3.1",
    release_url: "https://github.com/tinyhumansai/tinyjuice/releases/tag/v0.3.1",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyjuice-module-0.3.1-ubuntu-24.04-x86_64.tar.gz",
            sha256: "587bfbb669774265534910279d5fc48ac5df3eda47e69ec5ad0464ded346e2ac",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyjuice-module-0.3.1-ubuntu-24.04-arm64.tar.gz",
            sha256: "9167c17015c666ceda1f9bbacbb4f26853cb210359c425cd6ce68a0a6cafd90f",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyjuice-module-0.3.1-ubuntu-22.04-x86_64.tar.gz",
            sha256: "0cea5a1dc77e5c678986b098442de54c55006fd221ca2447df641f56d8d62518",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyjuice-module-0.3.1-ubuntu-22.04-arm64.tar.gz",
            sha256: "5465d8aac04b2c1bac3afbdd62cfe7da6a2a442197ed75028da45dcf5afd4f2a",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyjuice-module-0.3.1-macos-26-arm64.tar.gz",
            sha256: "5932862fe23a8a30679084d6f7623a682fa7e6aebda1d9227916dbb980131ace",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyjuice-module-0.3.1-macos-26-x86_64.tar.gz",
            sha256: "d2377ee6451afbc400d0ee4ab95dd834e627afcacd95d777c4b137a0f498998d",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyjuice-module-0.3.1-macos-15-arm64.tar.gz",
            sha256: "c3a36786ffb95b0444fa0dcf784124fa6b9cc49d97d121b35036f8e371124dcc",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyjuice-module-0.3.1-macos-15-x86_64.tar.gz",
            sha256: "db6d0b8baca62157605d01a61ec2d91a2f3ff6f2ae9aa328fa0ffeb0dffa74ef",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyjuice-module-0.3.1-windows-2025-x86_64.zip",
            sha256: "2c3f46ca6c180b940dcf368446a21c80b49ec82a17cfaf5c70ce8d5c54a678e5",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyjuice-module-0.3.1-windows-2022-x86_64.zip",
            sha256: "97c0b23e28068ef69cedf229ae65e1979917441bf8e2fcfd998806fc8ca29c67",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyjuice-module-0.3.1-windows-11-arm64.zip",
            sha256: "a68ebf0e9cf277cc85ce25a37e22ccc6bd8d91d3e1ed30bbb5b1ec1527f0c1dc",
        },
    ],
    load: LoadPolicy::Lazy,
};
