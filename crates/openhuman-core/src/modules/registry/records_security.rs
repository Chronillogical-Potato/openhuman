//! Published TinySecurity admission pins copied verbatim from v0.18.0/checksum.toml.
//!
//! `LoadPolicy::Eager` is the "load at boot" policy (the issue calls it Boot): a missing
//! security module changes behaviour, so it must be serving before the first request.
use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset, SourceSpec};

pub(crate) const TINYSECURITY: ModuleRecord = ModuleRecord {
    id: "tinysecurity",
    description: "Native security policy and durable approval tool grants",
    bus_name: tinysecurity_bus::names::INTERFACE,
    object_path: tinysecurity_bus::names::OBJECT_PATH,
    version: "0.18.1",
    release_url: "https://github.com/senamakel/tinysecurity/releases/tag/v0.18.1",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinysecurity-module-0.18.1-ubuntu-24.04-x86_64.tar.gz",
            sha256: "761f2a865a7cdf75e6a74a34d74050b484c88d9b8dfefc855890563ade9791d0",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinysecurity-module-0.18.1-ubuntu-24.04-arm64.tar.gz",
            sha256: "4bd3053b72a2ca8680b204b7b4ed7dd42d78b84101f0a627c0ca454df74ead3a",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinysecurity-module-0.18.1-ubuntu-22.04-x86_64.tar.gz",
            sha256: "6c7188e4758ceb5dd725f89f6063b05dcb7f4827789aa688e3e1baa823abbf9f",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinysecurity-module-0.18.1-ubuntu-22.04-arm64.tar.gz",
            sha256: "5e866d1a1a0261a7043fe18584adf617046c8433cf4398b12e2473b40f8bc129",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinysecurity-module-0.18.1-macos-26-arm64.tar.gz",
            sha256: "50c2cfeebdb36be377fac35c930e07ca9220534cfac321c533d1b12f22618ff0",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinysecurity-module-0.18.1-macos-26-x86_64.tar.gz",
            sha256: "dbb9186a7d74db98b0110a3efa54954e80e4add667f1dec1529c40d47a153bda",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinysecurity-module-0.18.1-macos-15-arm64.tar.gz",
            sha256: "0d022eaa0050bcabd0e68a8fb8d0decc1f3cf5c56f923157673abc952964c6d8",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinysecurity-module-0.18.1-macos-15-x86_64.tar.gz",
            sha256: "450f4aae95db218c72ad32965e846b5dab2ef8c494f97088c5e35a82bb5cb2c5",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinysecurity-module-0.18.1-windows-2025-x86_64.zip",
            sha256: "11fd5090ac90364c45d1596db37c583498f7f9bd838f1c091ca4f660c8466e59",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinysecurity-module-0.18.1-windows-2022-x86_64.zip",
            sha256: "83c6e894bc0b1811287594c4536975f34a0394e2d7428d3d38dbb2b30fa137eb",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinysecurity-module-0.18.1-windows-11-arm64.zip",
            sha256: "d71b55c54324a03c65790a583b97e0c942edc7a5074c484382d39d14e8bc4e11",
        },
    ],
    load: LoadPolicy::Eager,
    source: SourceSpec {
        submodule: "tinysecurity",
        manifest: "crates/tinysecurity-module/Cargo.toml",
        package: "tinysecurity-module",
        lib_name: "tinysecurity_module",
        features: &[],
    },
};
