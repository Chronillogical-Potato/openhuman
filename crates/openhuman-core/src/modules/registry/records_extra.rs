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
    version: "0.1.10",
    release_url: "https://github.com/tinyhumansai/tinychannels/releases/tag/v0.1.10",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinychannels-module-0.1.10-ubuntu-24.04-x86_64.tar.gz",
            sha256: "7b4cf71ded75876d770850223245d30a909788a298af858e858c50bce4f87e95",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinychannels-module-0.1.10-ubuntu-24.04-arm64.tar.gz",
            sha256: "2a755576a874f349d1827ee1afc698c6f9521843c1f06c1197ce62fec59568ed",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinychannels-module-0.1.10-ubuntu-22.04-x86_64.tar.gz",
            sha256: "c7f4943f01fa1af2b46637cb6907e79067b6c6763e2a9850398123beb4aa459f",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinychannels-module-0.1.10-ubuntu-22.04-arm64.tar.gz",
            sha256: "32bd9760d8a9717c87a9e5aa4a832171aea8ae8b2189035ca2d943c155f8e34a",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinychannels-module-0.1.10-macos-26-arm64.tar.gz",
            sha256: "6947bb8a0c011c40a4ccce797451add6be7caca923501b39ef2d7ead874fdc77",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinychannels-module-0.1.10-macos-26-x86_64.tar.gz",
            sha256: "ecce989844432a3610dabdaf29d94bb60c0fb8ff0fed29a50755fd3e42493007",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinychannels-module-0.1.10-macos-15-arm64.tar.gz",
            sha256: "bd536e55835aab04495441ac19302d0f99e7366c77960e84b3308cfd8780cc9b",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinychannels-module-0.1.10-macos-15-x86_64.tar.gz",
            sha256: "95cc8906bc73d27709df6022988ef2337ce36fec9ab68daf07d9ddff8ed9651a",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinychannels-module-0.1.10-windows-2025-x86_64.zip",
            sha256: "a272609902aa01692a11198696d0d25c8acc6f52e2f231237a5f9ddc70166b8c",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinychannels-module-0.1.10-windows-2022-x86_64.zip",
            sha256: "17ce261f8a9b772d2c8a4265bb1b7c22e1687073b7898a9ab9b9f54b81e979a9",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinychannels-module-0.1.10-windows-11-arm64.zip",
            sha256: "3bfcf3159c2bd7be84a9e8cddc5cc5f5a68b9742ccfe3d9e7fbc59558c0a8eb6",
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
