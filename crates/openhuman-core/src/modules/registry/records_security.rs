//! Published TinySecurity admission pins copied verbatim from v0.17.1/checksum.toml.
//!
//! `LoadPolicy::Eager` is the "load at boot" policy (the issue calls it Boot): a missing
//! security module changes behaviour, so it must be serving before the first request.
use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset, SourceSpec};

pub(crate) const TINYSECURITY: ModuleRecord = ModuleRecord {
    id: "tinysecurity",
    description: "Native security policy and durable approval tool grants",
    bus_name: tinysecurity_bus::names::INTERFACE,
    object_path: tinysecurity_bus::names::OBJECT_PATH,
    version: "0.18.0",
    release_url: "https://github.com/tinyhumansai/tinysecurity/releases/tag/v0.18.0",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinysecurity-module-0.18.0-ubuntu-24.04-x86_64.tar.gz",
            sha256: "70a5010d2bfb5c452501ea32288c53bd237c4621d0e306f6652ecc95394a217c",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinysecurity-module-0.18.0-ubuntu-24.04-arm64.tar.gz",
            sha256: "03821450613b9acf4e6be88fe152618fb56692a05ff122b90ffff96f0feeae64",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinysecurity-module-0.18.0-ubuntu-22.04-x86_64.tar.gz",
            sha256: "da57058ae65ebb9c058b785f3df8a09bb05b9a93aa88a10a49b162f2a838df58",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinysecurity-module-0.18.0-ubuntu-22.04-arm64.tar.gz",
            sha256: "75675631e62ce45f9c12735eefb15b6edf3e1c22b5d96641e1c224015632bf40",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinysecurity-module-0.18.0-macos-26-arm64.tar.gz",
            sha256: "43e5767d44788f1b1900e987c93a74146fc9bbfaf293e774400430ab816da64d",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinysecurity-module-0.18.0-macos-26-x86_64.tar.gz",
            sha256: "2c39cc8ea6be9a0c2912029a77b9455ff25d3c3d711b66b315b8a3be045f5231",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinysecurity-module-0.18.0-macos-15-arm64.tar.gz",
            sha256: "44e08ca8b87aaf7de84e43a2aa214e202a0481e9ce2d494eb255a7a2173f6021",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinysecurity-module-0.18.0-macos-15-x86_64.tar.gz",
            sha256: "bd274ef60d62f4abda1152fade2e4b0a3a41c5a42b3e53c7dc78b8c2b763bcd1",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinysecurity-module-0.18.0-windows-2025-x86_64.zip",
            sha256: "f258a588e6757e567823c5037d51a223c0f618005d2d1fd208cdd75f0c9a752b",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinysecurity-module-0.18.0-windows-2022-x86_64.zip",
            sha256: "67a3c13f93d1221a707fd913ade75b57edc2903c891fc23503e953ac5fe5f9b1",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinysecurity-module-0.18.0-windows-11-arm64.zip",
            sha256: "35cfea949bbe5409353593fc96670ce7a758d71f1135e0d6a372a9b1e2cf88a9",
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
