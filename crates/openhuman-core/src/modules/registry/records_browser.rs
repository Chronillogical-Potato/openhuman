//! TinyBrowser release assets, pinned from its published `v0.2.2` checksum.toml.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

pub(crate) const TINYBROWSER: ModuleRecord = ModuleRecord {
    id: "tinybrowser",
    description: "Chrome browser automation",
    bus_name: tinybrowser_bus::names::INTERFACE,
    object_path: tinybrowser_bus::names::OBJECT_PATH,
    version: "0.2.2",
    release_url: "https://github.com/tinyhumansai/tinybrowser/releases/tag/v0.2.2",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinybrowser-0.2.2-ubuntu-24.04-x86_64.tar.gz",
            sha256: "0e6c6a91c796c0b4ed967ae8931e92f1554f8d55193845e1b143f3887db9f40e",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinybrowser-0.2.2-ubuntu-24.04-arm64.tar.gz",
            sha256: "e2096e7044039ca0d5f6ad7bb4d937f36397396b92d23858a0e8e674cdbdc15c",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinybrowser-0.2.2-ubuntu-22.04-x86_64.tar.gz",
            sha256: "e1ed0123fffc36cdbaee95bebaba3b9e01de89ca404922cde6bc94460f403614",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinybrowser-0.2.2-ubuntu-22.04-arm64.tar.gz",
            sha256: "ae2f236813c421c8b2b9e42b5d6fb0c2c45d7c3ac8bffa63210ec592b2f8d9c9",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinybrowser-0.2.2-macos-26-arm64.tar.gz",
            sha256: "26848f07bbeba9b25f1f85cbefd45c1593750d209859cc705c093afd24ddda71",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinybrowser-0.2.2-macos-26-x86_64.tar.gz",
            sha256: "6e5fa9047f6592adb7e0279503b8c8f7970f21608ae9b5f45705a831323ed757",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinybrowser-0.2.2-macos-15-arm64.tar.gz",
            sha256: "590d5eefda37571c6b9c475c111b533c58f7db74b0fc32c31e3439b49017904f",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinybrowser-0.2.2-macos-15-x86_64.tar.gz",
            sha256: "27704e33d53d8ea0ea84555ca54f2ae9d24d88cc776cf1da73fb9309659b9ba6",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinybrowser-0.2.2-windows-2025-x86_64.zip",
            sha256: "2fee5e5d809cc8f09cf676fb115521aace93d9e2f1a3b9abaac5f7c9efff8ed6",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinybrowser-0.2.2-windows-2022-x86_64.zip",
            sha256: "68a49521e6da7e933f8960a38589fe24a87f08edc904e8a093f066c71a641fe6",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinybrowser-0.2.2-windows-11-arm64.zip",
            sha256: "bb2551d8588883773b57fa9e9d7cc4507d6f9dbbdb08729a098a4bd4bdb43338",
        },
        PlatformAsset {
            host_key: "fedora-43-x86_64",
            archive: "tinybrowser-0.2.2-fedora-43-x86_64.tar.gz",
            sha256: "f46210b86ffea672b7292f02e9846b7920423138587b3a5dcc04b62f693d5709",
        },
        PlatformAsset {
            host_key: "fedora-43-arm64",
            archive: "tinybrowser-0.2.2-fedora-43-arm64.tar.gz",
            sha256: "ceacb69ca11f25dd146451b7ae7fc4768954ba34a3f1dec382a27588eeca5dbe",
        },
        PlatformAsset {
            host_key: "fedora-44-x86_64",
            archive: "tinybrowser-0.2.2-fedora-44-x86_64.tar.gz",
            sha256: "9d77a334a189071d93fdbf82718b4413d48b28b0afb12fda4fbb7cc146de01f9",
        },
        PlatformAsset {
            host_key: "fedora-44-arm64",
            archive: "tinybrowser-0.2.2-fedora-44-arm64.tar.gz",
            sha256: "e19fee6afa5e833e429409b8120ac1a1f2d8b334b22949a0b4c39280f2425afe",
        },
        PlatformAsset {
            host_key: "archlinux-rolling-x86_64",
            archive: "tinybrowser-0.2.2-archlinux-rolling-x86_64.tar.gz",
            sha256: "991076c1f7b44ad8ab14a44f95694fcf5d07ac7423198e5564e0623db0422441",
        },
    ],
    load: LoadPolicy::Lazy,
};
