//! Registry records for the `tinymemory` and `tinyjuice` modules.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The complete TinyMemory engine, loaded eagerly so its capabilities are
/// available when the kernel assembles its RPC and tool surfaces.
pub(crate) const TINYMEMORY: ModuleRecord = ModuleRecord {
    id: "tinymemory",
    description: "Local memory engine: store, ranked recall, and portable export",
    bus_name: "ai.tinyhumans.tinymemory.Memory",
    object_path: "/ai/tinyhumans/tinymemory/Memory",
    version: "1.16.1",
    release_url: "https://github.com/tinyhumansai/tinymemory/releases/tag/v1.16.1",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinymemory-module-1.16.1-ubuntu-24.04-x86_64.tar.gz",
            sha256: "6afe17e3edd80e46538860e9445bfc45d205b8814587d49062117cb4da6edec8",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinymemory-module-1.16.1-ubuntu-24.04-arm64.tar.gz",
            sha256: "1348cd626b1ed109270ce801aeb4e68178d08daefaf79f142ccccc82bb9f1944",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinymemory-module-1.16.1-ubuntu-22.04-x86_64.tar.gz",
            sha256: "c025f4a5743bb975ed7cdc5306a16a6aa4ac2ef3c87fa83820d038321123daee",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinymemory-module-1.16.1-ubuntu-22.04-arm64.tar.gz",
            sha256: "27bc9694b468b7d7597e3259945148ce6fe39fec4f558dac5de1a554b1508169",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinymemory-module-1.16.1-macos-26-arm64.tar.gz",
            sha256: "dcd45c7e030ef01d6a48174c57aea48adfebb221144fb579c18180f068ec13df",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinymemory-module-1.16.1-macos-26-x86_64.tar.gz",
            sha256: "e2cdc685ecb24ad83ab98a1ce88be4b2e563535219eee8609a91ec1bcc5ac490",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinymemory-module-1.16.1-macos-15-arm64.tar.gz",
            sha256: "fd3c002707011ab594b3ac73114230aed570a6ebc9215e7c915d3024b74d8a19",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinymemory-module-1.16.1-macos-15-x86_64.tar.gz",
            sha256: "63030e318b20a410ea5f0dc8addd6c76a7f1848a45c8badb4e2c96f3f8a0539a",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinymemory-module-1.16.1-windows-2025-x86_64.zip",
            sha256: "94f902a44928d4485a62b5c7531dd91985faa3da466748777d8f1c0f3227eeaa",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinymemory-module-1.16.1-windows-2022-x86_64.zip",
            sha256: "d0f1aea80b3ee1b96eeeb1663b45ffdfd5ca32e09ce9b8653864c8fafc49e0b5",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinymemory-module-1.16.1-windows-11-arm64.zip",
            sha256: "3c287618965a203d1165490ca05693315e523008ad94acaafacb6c2dc998b041",
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
    version: "0.3.2",
    release_url: "https://github.com/tinyhumansai/tinyjuice/releases/tag/v0.3.2",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyjuice-module-0.3.2-ubuntu-24.04-x86_64.tar.gz",
            sha256: "69ade6a1a8145089a2edf4689505a100b4d9ce0cc4e507715cfac70d1fd09422",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyjuice-module-0.3.2-ubuntu-24.04-arm64.tar.gz",
            sha256: "2fe38fa593f3eaabcd4fa533998d9b07c9b637ada6a560ac08e9590f121fc9bb",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyjuice-module-0.3.2-ubuntu-22.04-x86_64.tar.gz",
            sha256: "48d32696eb9acacd6399a0e212291b48d1b31ecd3492109b479d67051446bf61",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyjuice-module-0.3.2-ubuntu-22.04-arm64.tar.gz",
            sha256: "b38b62960f4e81b277d6d30b1a4bf0d89b8e15fc5281b71ca6501187bd5af4b0",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyjuice-module-0.3.2-macos-26-arm64.tar.gz",
            sha256: "c304121196373b30aee90a79545d8e51779cf31a7e6d2a615e63b23dd7975ccb",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyjuice-module-0.3.2-macos-26-x86_64.tar.gz",
            sha256: "c072690d950221e84156a044d848c2fd61728a1930d0417275fa0953c246aea8",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyjuice-module-0.3.2-macos-15-arm64.tar.gz",
            sha256: "6d8fd67683c9485932059f9e1dd7f7af6ae3c9306e595e85cd09f72a903d7637",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyjuice-module-0.3.2-macos-15-x86_64.tar.gz",
            sha256: "14f6144479ebac26be8d14811b92379974e8ba0c3b7e6b5619e7b2392db06685",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyjuice-module-0.3.2-windows-2025-x86_64.zip",
            sha256: "2a6e85fcdacbf920198b68dca86c6ab746010d0029fe8f15594d00061b2a42ea",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyjuice-module-0.3.2-windows-2022-x86_64.zip",
            sha256: "f9a3aa66e6525617e9c803ac8ffc155e9181c39d3a123be370b05ae5247097ca",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyjuice-module-0.3.2-windows-11-arm64.zip",
            sha256: "81ce5ae65d91d1f6c33a2840079666915012e2d750362d8d5a3527c6466e1fd2",
        },
    ],
    load: LoadPolicy::Lazy,
};
