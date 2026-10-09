//! Registry records for additional first-party TinyBus modules.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The `tinybox` module, loaded on demand.
pub(crate) const TINYBOX: ModuleRecord = ModuleRecord {
    id: "tinybox",
    description: "Sandbox capability discovery through TinyBox",
    bus_name: "ai.tinyhumans.tinybox.Box",
    object_path: "/ai/tinyhumans/tinybox/Box",
    version: "0.1.15",
    release_url: "https://github.com/tinyhumansai/tinybox/releases/tag/v0.1.15",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinybox-0.1.15-ubuntu-24.04-x86_64.tar.gz",
            sha256: "43cd2cc0e9e5733d206be45d4c28e5c007862c48b5a3942a32d95e3995ba19cf",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinybox-0.1.15-ubuntu-24.04-arm64.tar.gz",
            sha256: "9cafb08666bb479ce42adc37e003fb9dbca8f9b9b8d36880ec8dd3c67afa1f10",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinybox-0.1.15-ubuntu-22.04-x86_64.tar.gz",
            sha256: "4b55df76c83a626959d6c130b9c263a8d0c062a789c6566e1d920d9230dd3209",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinybox-0.1.15-ubuntu-22.04-arm64.tar.gz",
            sha256: "81a4e6e56434729ce924ef94e66d5200f1cb66ccbd512f40f1bfbed749595682",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinybox-0.1.15-macos-26-arm64.tar.gz",
            sha256: "9433c307e7e5ee827b04d31dec771b8f28012ed6086d03abccca43deb99ecd50",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinybox-0.1.15-macos-26-x86_64.tar.gz",
            sha256: "18309b6692229b8abbecbfcb938d3e2cd509a58b067d44807355be40d48454a2",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinybox-0.1.15-macos-15-arm64.tar.gz",
            sha256: "fcbb3e7339c93fc8ee939ecc7fd38f1adf37c29ba322d4085705ae82afd049dc",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinybox-0.1.15-macos-15-x86_64.tar.gz",
            sha256: "314063db104b0353974c56232b28e63a8da57f438a72776d079145ceef1a0ea1",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinybox-0.1.15-windows-2025-x86_64.zip",
            sha256: "0acd1af4e5f19c75e2ef3b914d08717a0c1513a571b82bf5f48e32267f99fb31",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinybox-0.1.15-windows-2022-x86_64.zip",
            sha256: "d36da0d16217e3415c5a60950f5fa555c81f5e0ebd0552022d1a85ad48a3a2b1",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinybox-0.1.15-windows-11-arm64.zip",
            sha256: "d5ec94330ffc439451a05ebf5f2cd603236df5003993d34788deb4ac5fe22f49",
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
