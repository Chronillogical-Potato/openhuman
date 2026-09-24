//! Registry records for the `tinymcp` and `tinyconnectors` modules.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The `tinymcp` module: the Model Context Protocol client.
///
/// Owns both transports (Streamable HTTP and a subprocess over stdio), the
/// statically declared server set a host puts in its own configuration, the
/// dynamic registry of user-installed servers with its SQLite store, the
/// reconnect supervisor, the browser sign-in flow, and the write-audit log.
///
/// Lazy, because dialing an MCP server is something most sessions never do: a
/// host with no installed servers and no configured ones would otherwise pay a
/// download and a `dlopen` for a capability it never reaches. That differs from
/// the module's own `lazy = false` export hint, which speaks for a host whose
/// servers should be connected the moment it comes up — this host decides when
/// that moment is, and does so on the first ask.
///
/// **What stays out of the module is host policy**, and the split is the same
/// one the contract's own documentation draws: the prompt-injection scan over
/// remote tool definitions, the `mcp_clients` RPC surface, the
/// agent-facing tools, and the proxy *scoping* decision all belong to this
/// application's threat model, not to a protocol client. `tinymcp-bus` carries
/// the vocabulary; this table says which bytes may speak it.
pub(crate) const TINYMCP: ModuleRecord = ModuleRecord {
    id: "tinymcp",
    description: "Model Context Protocol client: transports, registry, and the write-audit log",
    bus_name: "ai.tinyhumans.tinymcp.Mcp",
    object_path: "/ai/tinyhumans/tinymcp/Mcp",
    version: "0.3.3",
    release_url: "https://github.com/tinyhumansai/tinymcp/releases/tag/v0.3.3",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinymcp-0.3.3-ubuntu-24.04-x86_64.tar.gz",
            sha256: "7c49adafbdeed02b5555d35aafb9e0b1d6a27ea6586aa24518d19d58dc21079b",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinymcp-0.3.3-ubuntu-24.04-arm64.tar.gz",
            sha256: "996637662a3406681eae477d2d333312894a9709c83fa179a7aacfe068ba8be0",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinymcp-0.3.3-ubuntu-22.04-x86_64.tar.gz",
            sha256: "82da931e0e5e9e82feb189042ac130fa37f3ae5ffec8d935ddd89b41859ef55c",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinymcp-0.3.3-ubuntu-22.04-arm64.tar.gz",
            sha256: "a69a995808c0098ce80ef9a38d590b829e83abd9370410807cf482f0ae77097a",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinymcp-0.3.3-macos-26-arm64.tar.gz",
            sha256: "5fefcc5a3ec34dbd4bcee70615fff456be9cf28a74c1d06d07f810137facf5d7",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinymcp-0.3.3-macos-26-x86_64.tar.gz",
            sha256: "c04b9ec6746f3317c695cde897a168101a6ea9fe1fe9b5e08a85aba3a2429baa",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinymcp-0.3.3-macos-15-arm64.tar.gz",
            sha256: "67172f3d6be08a57ee5e83039d8100be60132e1bbf32d027f6b2eb150eb59b3f",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinymcp-0.3.3-macos-15-x86_64.tar.gz",
            sha256: "6eac1dd3e55a0e6ad5341c30182f24fd5ea0bba7cce07ecec17c607aea40882c",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinymcp-0.3.3-windows-2025-x86_64.zip",
            sha256: "e9b7dd357deddc3e659eccac1eeb9a9bc018f083b3120de2bb36027b7824cd9b",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinymcp-0.3.3-windows-2022-x86_64.zip",
            sha256: "961cf051976f8f650270764b3ad11ced8067d1a0f953c091ed4b9da524e18051",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinymcp-0.3.3-windows-11-arm64.zip",
            sha256: "312eda791a4cc3fa9e8ef46415c8471ba280aea63534718486a92ee764debe61",
        },
    ],
    load: LoadPolicy::Lazy,
};

pub(crate) const TINYCONNECTORS: ModuleRecord = ModuleRecord {
    id: "tinyconnectors",
    description: "OAuth connector integrations: accounts, actions, triggers, and record sync",
    bus_name: "ai.tinyhumans.connectors.Composio",
    object_path: "/ai/tinyhumans/connectors/Composio",
    version: "0.10.1",
    release_url: "https://github.com/tinyhumansai/tinyconnectors/releases/tag/v0.10.1",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyconnectors-0.10.1-ubuntu-24.04-x86_64.tar.gz",
            sha256: "21514543ee2937486426e584df7bb4e18cd57a8b1f00e68eb208856c3e8cbd24",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyconnectors-0.10.1-ubuntu-24.04-arm64.tar.gz",
            sha256: "51cdc6708de5fa872877bbe6f2aa13761a24bb9d2e2d60eca4bcff06ea076db5",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyconnectors-0.10.1-ubuntu-22.04-x86_64.tar.gz",
            sha256: "db96c19fa08dbc5f3e37695b6305a750883d849949ed55245ab59975ff9b6238",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyconnectors-0.10.1-ubuntu-22.04-arm64.tar.gz",
            sha256: "686a76fe89466d0d7b3f264d68fcdd3dde4e34adb5193eae8ed236a0a04e02d7",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyconnectors-0.10.1-macos-26-arm64.tar.gz",
            sha256: "fe2ab2f092503ce9b0046aae155fe799ef09bfb4c8be040270a006c9e5ce270a",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyconnectors-0.10.1-macos-26-x86_64.tar.gz",
            sha256: "44eea082e582c68688e384f35a52a730365052db39adcad69ab9fafb6bd81fa3",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyconnectors-0.10.1-macos-15-arm64.tar.gz",
            sha256: "0cad041a4ece339be5a03b6f8f8c50e2f515082903d291de440651478385421e",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyconnectors-0.10.1-macos-15-x86_64.tar.gz",
            sha256: "c938b4cdc9ca832eba57af9698d500ea799baa885cac12d1e91705a41d6a4b90",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyconnectors-0.10.1-windows-2025-x86_64.zip",
            sha256: "bf857b8767cddb6a72c848fe5e1aa4ffe06868f7e8c2fe477372edce6d4e1788",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyconnectors-0.10.1-windows-2022-x86_64.zip",
            sha256: "0cb2cc73b262962adfa6fad2c482279e8f448e531712927a882902f7dcee2adb",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyconnectors-0.10.1-windows-11-arm64.zip",
            sha256: "88e4860d165fc92c4856f489115e5faa07ffdd85bda2260b9bf84ab498c42331",
        },
    ],
    // Lazy: a user with no connected accounts should not pay to load it, and
    // most sessions never touch a connector. Safe even signed out — the module
    // loads without configuration and still answers the capability members.
    load: LoadPolicy::Lazy,
};
