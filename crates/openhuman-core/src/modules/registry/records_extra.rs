//! Registry records for additional first-party TinyBus modules.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The `tinybox` module, loaded on demand.
pub(crate) const TINYBOX: ModuleRecord = ModuleRecord {
    id: "tinybox",
    description: "Sandbox capability discovery through TinyBox",
    bus_name: "ai.tinyhumans.tinybox.Box",
    object_path: "/ai/tinyhumans/tinybox/Box",
    version: "0.1.16",
    release_url: "https://github.com/tinyhumansai/tinybox/releases/tag/v0.1.16",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinybox-0.1.16-ubuntu-24.04-x86_64.tar.gz",
            sha256: "3163f7cf621beaf71d99b978555085e6bb933a0810153970dd16b2c531e1a030",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinybox-0.1.16-ubuntu-24.04-arm64.tar.gz",
            sha256: "fad323bf74ce075f758a31c7cb93f393d84ac4c9a6a22f31fb7fc7e2ec4743c4",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinybox-0.1.16-ubuntu-22.04-x86_64.tar.gz",
            sha256: "9abddc0e8ad14ac9f34e479714baa7f1720ed029d199272214450f356b7d51da",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinybox-0.1.16-ubuntu-22.04-arm64.tar.gz",
            sha256: "fb164eeec76789035de8c621176630b6ea6aea0a5021778fbd97fafe163b6dac",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinybox-0.1.16-macos-26-arm64.tar.gz",
            sha256: "34cb87457a3b21ec5ee8b8b6817f6a78a854b12e4d40fa43221aeed508d7c40a",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinybox-0.1.16-macos-26-x86_64.tar.gz",
            sha256: "5f7a4886fb191a58dc33bb5f43a7034c0f6b31c1e85161e13228aa0deb6ae5b9",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinybox-0.1.16-macos-15-arm64.tar.gz",
            sha256: "ae427b7962b0607051595c9c12f9cb630c87672bc3c92c7becdac87283aaefd4",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinybox-0.1.16-macos-15-x86_64.tar.gz",
            sha256: "417d4c182c3882cd90b5ff323dc81c62c2d98b3e0bb8ac0a237cac51c34828fe",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinybox-0.1.16-windows-2025-x86_64.zip",
            sha256: "ada7ddb1edf16887ef20d84241f0453b2ce8ee29aa670c1505758c1f7913b4fc",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinybox-0.1.16-windows-2022-x86_64.zip",
            sha256: "044666f53b10c8db6626262de45806d1a2a34147ce2de196e8549987f32c1cd8",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinybox-0.1.16-windows-11-arm64.zip",
            sha256: "bc3da524fc3e702e77cc1e85c064dccaece633f896503737c71ce7127ba26440",
        },
    ],
    load: LoadPolicy::Lazy,
};

/// The `tinychannels` module, loaded on demand.
pub(crate) const TINYCHANNELS: ModuleRecord = ModuleRecord {
    id: "tinychannels",
    description: "Channel provider lifecycle and message transport",
    bus_name: "ai.tinyhumans.tinychannels.Channels",
    object_path: "/ai/tinyhumans/tinychannels/Channels",
    version: "0.1.12",
    release_url: "https://github.com/tinyhumansai/tinychannels/releases/tag/v0.1.12",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinychannels-module-0.1.12-ubuntu-24.04-x86_64.tar.gz",
            sha256: "4d238ccecc5e2ac40006e17dfe51eeae3e9d5504395db852d83037f72aaa296f",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinychannels-module-0.1.12-ubuntu-24.04-arm64.tar.gz",
            sha256: "9474764c23ed7e3e83ad28611d0e2d403441ee0c8d6f2f643b0e7c190b0065d1",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinychannels-module-0.1.12-ubuntu-22.04-x86_64.tar.gz",
            sha256: "deb8313e1c5607b83d314b61c2b6305f104a6c0dd5ce19f7ef65018d4d3f78b0",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinychannels-module-0.1.12-ubuntu-22.04-arm64.tar.gz",
            sha256: "3f1a85cce6e19959f9ba29829f82c8bd195f553d4fd2b11d9f4de9fdf7243604",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinychannels-module-0.1.12-macos-26-arm64.tar.gz",
            sha256: "64131eb839e356cb1e3519cd0b10d6ab355ff31330f4b4531523567e3de7c577",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinychannels-module-0.1.12-macos-26-x86_64.tar.gz",
            sha256: "c92985145ca92b52a4af2ae7e2a881d4f4fa2aa78bd285bede4399459a8a4153",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinychannels-module-0.1.12-macos-15-arm64.tar.gz",
            sha256: "bdd48673381b80997a6916d85912fa6fbd6aae477b77ff296524bf9297b6abd8",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinychannels-module-0.1.12-macos-15-x86_64.tar.gz",
            sha256: "8c595292d9ac46d2af0c81c22a2856b0f7fc22f9dcc8bbd567bbc410b5073954",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinychannels-module-0.1.12-windows-2025-x86_64.zip",
            sha256: "4ccddd11a8057e18a55a924bf46d4670c3a34ba0e1a7fccd95e848504d24d3cf",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinychannels-module-0.1.12-windows-2022-x86_64.zip",
            sha256: "e856df1a46c8e966325075320e37d1443671058c23c46ee517c65b6f42912abf",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinychannels-module-0.1.12-windows-11-arm64.zip",
            sha256: "c9c5167fd8c19581f8b15d68cc2871627ac3006573d0ee40e709ca16bb21f109",
        },
    ],
    load: LoadPolicy::Lazy,
};

/// The `tinyhosts` module, loaded on demand.
pub(crate) const TINYHOSTS: ModuleRecord = ModuleRecord {
    id: "tinyhosts",
    description: "Hosting provider operations",
    bus_name: "ai.tinyhumans.tinyhosts.Hosting",
    object_path: "/ai/tinyhumans/tinyhosts/Hosting",
    version: "0.2.2",
    release_url: "https://github.com/tinyhumansai/tinyhosts/releases/tag/v0.2.2",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyhosts-0.2.2-ubuntu-24.04-x86_64.tar.gz",
            sha256: "92e4176fc3e4d23f0d6b8395a6596ca7f548602eedff9a08746e1a934f7a91a7",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyhosts-0.2.2-ubuntu-24.04-arm64.tar.gz",
            sha256: "de5c2cd3b0ba8f350b658664bbff0937bcc4121451f8e706401a76c2b3c785a9",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyhosts-0.2.2-ubuntu-22.04-x86_64.tar.gz",
            sha256: "8a61cd86e6c3812a84f0a3b473221352fd6f4b2045dd7b5dbef43cfad2fa189b",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyhosts-0.2.2-ubuntu-22.04-arm64.tar.gz",
            sha256: "2cbe7797290b4cc16be08a3885b7abbc2a870f2822cee8f746dd1c2ecc74d796",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyhosts-0.2.2-macos-26-arm64.tar.gz",
            sha256: "afeba2bb84316f6e0bfdbae55dea3a4214e9b2bdf5a623139239d5b2f94b4a51",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyhosts-0.2.2-macos-26-x86_64.tar.gz",
            sha256: "8a23e7d910b47b24c4609f46af62d827c4439439a0b9b25e83356a53f7027695",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyhosts-0.2.2-macos-15-arm64.tar.gz",
            sha256: "d04c0778b253a6dce67d3bbdf54a02e1df594e664b29fab6aabbefbfad63dd77",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyhosts-0.2.2-macos-15-x86_64.tar.gz",
            sha256: "38842da342ad502b7a3eca6f0d10fe32bb5bfbe023ec4d39069934e61b133134",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyhosts-0.2.2-windows-2025-x86_64.zip",
            sha256: "afadbc834a2c0e77021365113a2af1d52c55159e98aebd0340d4c10a6b2532ac",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyhosts-0.2.2-windows-2022-x86_64.zip",
            sha256: "34d9302cf474e1f0e383fbd9d30b2da567de1b02458b6bfa9309671adddb361a",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyhosts-0.2.2-windows-11-arm64.zip",
            sha256: "27f1bd2da3b6bde602e0a3f12c104ef8569636d82643365c402e9777b081d4ad",
        },
    ],
    load: LoadPolicy::Lazy,
};
