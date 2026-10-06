//! Registry records for the `tinyruntime` router and its Node.js and Python
//! sidecars.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The `tinyruntime` module: the runtime router.
///
/// Resolves a language runtime, installs one when the host has none, reuses one
/// when it does, and runs code on a bounded pool of warm interpreter processes.
/// It is a router: on its own it knows no languages, and it routes to the two
/// provider records below.
///
/// Lazy, because a host that never runs a skill, a flow step, or a `node_exec`
/// should not pay a download and a `dlopen` for the ability to.
///
/// The digests below are v0.2.9's, taken verbatim from that release's
/// `checksum.toml`. Until it existed this record carried no assets at all and
/// the module was reachable only from a developer build named by
/// `modules.local` or found on `OPENHUMAN_MODULE_PATH` — so on any machine that
/// had not built it, the runtime domain was a set of tools that could not run.
pub(crate) const TINYRUNTIME: ModuleRecord = ModuleRecord {
    id: "tinyruntime",
    description: "Language runtime resolution, installation, and pooled execution",
    bus_name: "ai.tinyhumans.runtime.Runtime",
    object_path: "/ai/tinyhumans/runtime/Runtime",
    version: "0.2.11",
    release_url: "https://github.com/tinyhumansai/tinyruntime/releases/tag/v0.2.11",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyruntime-0.2.11-ubuntu-24.04-x86_64.tar.gz",
            sha256: "a7141e0e8484b2cb587e65c100dc821bbd3401b107b9faa71123f227b9eae73f",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyruntime-0.2.11-ubuntu-24.04-arm64.tar.gz",
            sha256: "8fa93f85498685007afc4c18c30679c31d30dae839502c03064fa08876c9e79b",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyruntime-0.2.11-ubuntu-22.04-x86_64.tar.gz",
            sha256: "4d9f3f6eb3d457a8004de448b1c1e550577909eccc7fec3d73f9b325d806bb89",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyruntime-0.2.11-ubuntu-22.04-arm64.tar.gz",
            sha256: "85926291283cb9d025caed0807c32f528d9ef15ce1ec18ccfc46ff66d0b25735",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyruntime-0.2.11-macos-26-arm64.tar.gz",
            sha256: "be6d532dda60b6c240f9be00199dedd66df64730d39f7acf87068dd065e64ad9",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyruntime-0.2.11-macos-26-x86_64.tar.gz",
            sha256: "bffb8c8355af9018d42962aa1744bc9b683409a541e2420f174b636747d488b3",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyruntime-0.2.11-macos-15-arm64.tar.gz",
            sha256: "078b436783c68ed8ad0cf2e9d32000006c429abda531da17da4c4baaa01462c1",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyruntime-0.2.11-macos-15-x86_64.tar.gz",
            sha256: "0251510360de58a502361ed3e5497fbcc710ebcf60cea432ecba8b83ec1455be",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyruntime-0.2.11-windows-2025-x86_64.zip",
            sha256: "0b2ef8553447389dfc6dcea53085beb30f858cdbcaa006d3e36650fa159f5f70",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyruntime-0.2.11-windows-2022-x86_64.zip",
            sha256: "0859f1ec61d20a3d0d7e85f23439d44537cf378b39044f9433a22e9f5aef595b",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyruntime-0.2.11-windows-11-arm64.zip",
            sha256: "f3520cbfdfbb734ac48ea5ba14636676e8e94eda65ebaede5c398f9de9a2267e",
        },
    ],
    load: LoadPolicy::Lazy,
};

/// The `tinyruntime-nodejs` module: the Node.js half of the router's knowledge.
///
/// Answers which host interpreters count, which archive nodejs.org publishes for
/// this machine, where the binaries land, and what a warm Node worker is. It
/// installs nothing itself.
///
/// It implements the shared `ai.tinyhumans.runtime.Provider` interface but
/// serves at its own object path, because two modules cannot claim one bus name
/// and tinybus derives the path from the name.
///
/// Lazy, and loaded by the same call that loads the router: a language is only
/// worth its `dlopen` when something asks for that language.
///
/// Released from its own repository (own version line) against the router's source pin; see scripts/ci/module-provider-pins.json.
pub(crate) const TINYRUNTIME_NODEJS: ModuleRecord = ModuleRecord {
    id: "tinyruntime-nodejs",
    description: "Node.js runtime provider for tinyruntime",
    bus_name: "ai.tinyhumans.runtime.nodejs.Provider",
    object_path: "/ai/tinyhumans/runtime/nodejs/Provider",
    version: "0.2.6",
    release_url: "https://github.com/tinyhumansai/tinyruntime-nodejs/releases/tag/v0.2.6",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyruntime-nodejs-0.2.6-ubuntu-24.04-x86_64.tar.gz",
            sha256: "ed1344095b12a9b46f1803c096695d2419466809df2675e971155d963b7a09e6",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyruntime-nodejs-0.2.6-ubuntu-24.04-arm64.tar.gz",
            sha256: "834854cd05434258519a89095ef16f4d2da45b0bbbeaaf276df25b6ab811934e",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyruntime-nodejs-0.2.6-ubuntu-22.04-x86_64.tar.gz",
            sha256: "eff7d293b36320cbeeaaa0c2b0be0a76f9532a588cec596825fff44b4c3986ce",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyruntime-nodejs-0.2.6-ubuntu-22.04-arm64.tar.gz",
            sha256: "599ae41f3724b1631c2ec67a124dd958aa1f6301ab4a073bb50a19071a54ec56",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyruntime-nodejs-0.2.6-macos-26-arm64.tar.gz",
            sha256: "8d2ae6bf503624d078ba539c2e94aa0510d0d1d08d409fb44138f8ce29a407a6",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyruntime-nodejs-0.2.6-macos-26-x86_64.tar.gz",
            sha256: "d30fae190d8fa352f732810e50a876046ace5f09b895dbec2d8431f094ac4a9f",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyruntime-nodejs-0.2.6-macos-15-arm64.tar.gz",
            sha256: "e43ee543254f6baacb4659faca05524169cb7eb642d32be0bf7f879339c07b43",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyruntime-nodejs-0.2.6-macos-15-x86_64.tar.gz",
            sha256: "15a29d3362667780fb145dca759273edc420d3eb0ef1479df8a02645f463a7d9",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyruntime-nodejs-0.2.6-windows-2025-x86_64.zip",
            sha256: "62af388752e5b3dcf25596b9bb7d7eefd62fc17aba0ebf2a4792ca8a984c88fd",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyruntime-nodejs-0.2.6-windows-2022-x86_64.zip",
            sha256: "e3da23f53f712c058d58ce58c95f5158e8207d7e84110e6462aac5fae03394b8",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyruntime-nodejs-0.2.6-windows-11-arm64.zip",
            sha256: "0c5887184fa5b1b6d50411996a5fb697be06b42635dcb23263ba111859b20fc0",
        },
    ],
    load: LoadPolicy::Lazy,
};

/// The `tinyruntime-python` module: the Python half of the router's knowledge.
///
/// Answers which host interpreters count, which standalone build to install, and
/// what a warm Python worker is. It installs nothing itself.
///
/// Released from its own repository (own version line) against the router's source pin; see scripts/ci/module-provider-pins.json.
pub(crate) const TINYRUNTIME_PYTHON: ModuleRecord = ModuleRecord {
    id: "tinyruntime-python",
    description: "Python runtime provider for tinyruntime",
    bus_name: "ai.tinyhumans.runtime.python.Provider",
    object_path: "/ai/tinyhumans/runtime/python/Provider",
    version: "0.2.6",
    release_url: "https://github.com/tinyhumansai/tinyruntime-python/releases/tag/v0.2.6",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyruntime-python-0.2.6-ubuntu-24.04-x86_64.tar.gz",
            sha256: "bf628568033bcec5bfcf5017b84eb4d09ee4733666a6ca0891baefc3b1d1e2ef",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyruntime-python-0.2.6-ubuntu-24.04-arm64.tar.gz",
            sha256: "2e3433d21a88902f48fa89ae5533e735a1655702c41ff7c0d717b03d968442b7",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyruntime-python-0.2.6-ubuntu-22.04-x86_64.tar.gz",
            sha256: "4c95c390e7357c636c3fe4a8d06edceb546ec46998f12b7028b148acc100b35f",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyruntime-python-0.2.6-ubuntu-22.04-arm64.tar.gz",
            sha256: "5421e577d3f96affa49422a758d032d5a46e124d4d9487f1e4c67a260d1398f3",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyruntime-python-0.2.6-macos-26-arm64.tar.gz",
            sha256: "8b78e7e13d9f2561e08161d1285b64ce4c1c94e5c1fd95a24afa510fec7aacc9",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyruntime-python-0.2.6-macos-26-x86_64.tar.gz",
            sha256: "88dd3d89a5dacb9f0872918ea4d8f672982cb9a6dfc8ada992a0d13f71674974",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyruntime-python-0.2.6-macos-15-arm64.tar.gz",
            sha256: "a65e6988026f0526e15eda5b4b39a5b011b4c8c11eea17dfb9cbc084f83da4a4",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyruntime-python-0.2.6-macos-15-x86_64.tar.gz",
            sha256: "287b58d7f61fb99aafbf12472a493e6ab4471a48f706c4d30c63c514b11e8798",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyruntime-python-0.2.6-windows-2025-x86_64.zip",
            sha256: "7726b96c52454ef72ce379f2e172dd36aa6f9b96705b9fe0cb6dc1720de30785",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyruntime-python-0.2.6-windows-2022-x86_64.zip",
            sha256: "9a5fa1381449e73994ab1a9af26a5e6bfac268a88041e9dee135303f1fb55255",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyruntime-python-0.2.6-windows-11-arm64.zip",
            sha256: "348db1f881e435c41c1dd81b562226543f414fc93aa07c8e92ac26c58cfc141f",
        },
    ],
    load: LoadPolicy::Lazy,
};
