//! TinySearch web-search module. Published digests are added only from its
//! release manifest.
use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

pub(crate) const TINYSEARCH: ModuleRecord = ModuleRecord {
    id: "tinysearch",
    description:
        "Web search, grounded answers and page contents across providers through TinySearch",
    bus_name: tinysearch_bus::names::INTERFACE,
    object_path: tinysearch_bus::names::OBJECT_PATH,
    version: "0.3.6",
    release_url: "https://github.com/tinyhumansai/tinysearch/releases/tag/v0.3.6",
    // Verbatim from the published v0.3.3 checksum.toml; the same host set as
    // the other native modules (macOS, Ubuntu, Windows).
    assets: &[
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinysearch-0.3.6-macos-26-arm64.tar.gz",
            sha256: "3acc45955bf17b0022a921ee7d2a3246f03b7d7cb8fc0ab0a1ed1bd12e8dcc0c",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinysearch-0.3.6-macos-26-x86_64.tar.gz",
            sha256: "fce444fb202cbc1a9f3f850187b9ed2a8e7597297c76ef28d0673a25e00b5f6d",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinysearch-0.3.6-macos-15-arm64.tar.gz",
            sha256: "07a3454fa19076290dcf0207fe3c9300e32e1da86a9b83ccacf0826f3e2cebe7",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinysearch-0.3.6-macos-15-x86_64.tar.gz",
            sha256: "4a90b0ac5f0b6e275e4b9896d7f367bcde4c70b06025f12960697e88e13b53bd",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinysearch-0.3.6-ubuntu-24.04-x86_64.tar.gz",
            sha256: "72c1cc0bdad04b0ff76b20f59f5eb6afa354b856869ba086727c22af620beea1",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinysearch-0.3.6-ubuntu-24.04-arm64.tar.gz",
            sha256: "ee01a6253a8d59a07f8211874be062da75cbb884a311dd222435056bb2e4ea23",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinysearch-0.3.6-ubuntu-22.04-x86_64.tar.gz",
            sha256: "82b1915d52f0e64300d3c97c1edd430948a209f5477db831aedb10e2d788da1e",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinysearch-0.3.6-ubuntu-22.04-arm64.tar.gz",
            sha256: "7982e1736dac1567def4c71b0e3c80dba49c70c8f93329f5eaa4109e861d490c",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinysearch-0.3.6-windows-2025-x86_64.zip",
            sha256: "e9daecbea876e1dd38f1d4fa66caf2323aabfa07328d10abb36cdfe729eddf42",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinysearch-0.3.6-windows-2022-x86_64.zip",
            sha256: "9ffda0883d61830b617c9fbc85a6a7bcefa8a36fc7b9965d406964b244eb9d52",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinysearch-0.3.6-windows-11-arm64.zip",
            sha256: "7cfb5622dba111824bac05eba490fd79fa94211123a0dff052be50ed4394d6a6",
        },
    ],
    load: LoadPolicy::Lazy,
};
