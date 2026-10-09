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
    version: "0.1.11",
    release_url: "https://github.com/tinyhumansai/tinychannels/releases/tag/v0.1.11",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinychannels-module-0.1.11-ubuntu-24.04-x86_64.tar.gz",
            sha256: "13719fc3e342b7ddcd64a0cd1f822f2be2e10932b8c90fe7af844b44a71d9cf3",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinychannels-module-0.1.11-ubuntu-24.04-arm64.tar.gz",
            sha256: "6736bcd486abde4b45728146369565a71418066b4ea3bd4c2008e3df054d3898",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinychannels-module-0.1.11-ubuntu-22.04-x86_64.tar.gz",
            sha256: "739080f8f2d7f73eb6d01bfd28724a63e9d3b05584c496699a056e119806212e",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinychannels-module-0.1.11-ubuntu-22.04-arm64.tar.gz",
            sha256: "116baf0b56afd88b483d68e97c6203e48f82bf818ca9c9e3d037d053979c1264",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinychannels-module-0.1.11-macos-26-arm64.tar.gz",
            sha256: "cea117017701f943faef94cee7272eac992a096dd51c45ee990f5afc896c7223",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinychannels-module-0.1.11-macos-26-x86_64.tar.gz",
            sha256: "167f2e5ceefb3abffd40694936b2175df25a7d26f73bc69a2c9c08d3062daee5",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinychannels-module-0.1.11-macos-15-arm64.tar.gz",
            sha256: "e8c33360d49bb87d0faf6162eeb8ce59456ec7ce3dc0496213dbb3c52c806714",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinychannels-module-0.1.11-macos-15-x86_64.tar.gz",
            sha256: "d7cd73b6ad9f1661457ab2d36850e392c8a9503d6e5a944d58aa9c740b046b72",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinychannels-module-0.1.11-windows-2025-x86_64.zip",
            sha256: "3687cdf82c7b271831b75d1f9d2b51ec6661c47daa31eaca048adcf503196c22",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinychannels-module-0.1.11-windows-2022-x86_64.zip",
            sha256: "7eed6504023a47d5319d19ee3fc923eb3da20a95c9516f6fcef00c420b61fa92",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinychannels-module-0.1.11-windows-11-arm64.zip",
            sha256: "9489f9bae3a5429151eca0530790b4579ef62cd921bcba59a42b9e64937cc4fe",
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
