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
    version: "0.4.0",
    release_url: "https://github.com/tinyhumansai/tinymcp/releases/tag/v0.4.0",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinymcp-0.4.0-ubuntu-24.04-x86_64.tar.gz",
            sha256: "0f2eb9aebbc5843b43e6855bc413674d6a573337d8eb4e6c5c3fbd9b16998c84",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinymcp-0.4.0-ubuntu-24.04-arm64.tar.gz",
            sha256: "4798f58671ec69b08768e3b3c15cf4f6d21a329379dde42a161288b94a6adf81",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinymcp-0.4.0-ubuntu-22.04-x86_64.tar.gz",
            sha256: "b696cecd0af5eff56a3abf8c1ebd65af413f1d7e32109ec2953170a05761e933",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinymcp-0.4.0-ubuntu-22.04-arm64.tar.gz",
            sha256: "1d3f3b535a11d0bc2af56d7ba1128d142ba6decd1c8ce654e312bc8e445b0386",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinymcp-0.4.0-macos-26-arm64.tar.gz",
            sha256: "c02bbaf4b4f66d435c072eef31f18179f1d2a39d739e29670a3fdd1d97cfd922",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinymcp-0.4.0-macos-26-x86_64.tar.gz",
            sha256: "c82beb265fa91742b99df50419bc587bc7be16107955614e45fed461a44d8631",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinymcp-0.4.0-macos-15-arm64.tar.gz",
            sha256: "8d8d5ee8a13eb65ea73f4058d9b0baf37fa4a00ad4bb4eb21101af85a09390cc",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinymcp-0.4.0-macos-15-x86_64.tar.gz",
            sha256: "6b1f0f0cdec82d9747ab7cc4363e00235fac83ee62863c2a2ed4bb996cae20c0",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinymcp-0.4.0-windows-2025-x86_64.zip",
            sha256: "16c842cd46f82860e72a055d83d13600ff07b32053e3d06d240932a9d95fbb9c",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinymcp-0.4.0-windows-2022-x86_64.zip",
            sha256: "d8bd15babf6288da2123856c90f99d63130d1738cc3eeab15491ad2219269d01",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinymcp-0.4.0-windows-11-arm64.zip",
            sha256: "1a00a3bf5c09edb2ed284cf62ec10d185fd3148c4ab8f832aa96d9f09a093ce8",
        },
    ],
    load: LoadPolicy::Lazy,
};

pub(crate) const TINYCONNECTORS: ModuleRecord = ModuleRecord {
    id: "tinyconnectors",
    description: "OAuth connector integrations: accounts, actions, and triggers",
    bus_name: "ai.tinyhumans.connectors.Composio",
    object_path: "/ai/tinyhumans/connectors/Composio",
    version: "0.13.0",
    release_url: "https://github.com/tinyhumansai/tinyconnectors/releases/tag/v0.13.0",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyconnectors-0.13.0-ubuntu-24.04-x86_64.tar.gz",
            sha256: "17f20ab28b2a16d6c25d1ab0ea3d31a77eae73acda32766257a431d8ca8c6db3",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyconnectors-0.13.0-ubuntu-24.04-arm64.tar.gz",
            sha256: "83c8ecb29498a624f95e9e08e11bd100cd97a93597da74c276dd9a3cdc7c9aad",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyconnectors-0.13.0-ubuntu-22.04-x86_64.tar.gz",
            sha256: "0d474605d666db28a14d7dae08efa7c9a3493148fa0f2ae78bac10dcc7f2a0ab",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyconnectors-0.13.0-ubuntu-22.04-arm64.tar.gz",
            sha256: "64dc9e0edf9511dd4d04f708b034d4e7b3ffcbc894adb1e53bb6a33b755914eb",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyconnectors-0.13.0-macos-26-arm64.tar.gz",
            sha256: "042ef6e041ccead4d4235ec99636f259696d9d22b12da2a2fe06b5e7dd4321c0",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyconnectors-0.13.0-macos-26-x86_64.tar.gz",
            sha256: "67b8c5bd732d0651ee896a7480e0a3cdeaa9f77549673e5ccb755be73981021d",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyconnectors-0.13.0-macos-15-arm64.tar.gz",
            sha256: "e5829fee588a29ebbd83c7a3f5a3a521e7345ca4f8f516fc23dafc2f85b72794",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyconnectors-0.13.0-macos-15-x86_64.tar.gz",
            sha256: "e0468fc8ce1aa6110762d51ddc2b0443cd76c173c37b730ce95a8d461367bb4e",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyconnectors-0.13.0-windows-2025-x86_64.zip",
            sha256: "3858be52153bbe0483fadaea8b418b90d90bf0d3da23da0bfbe6137e117bbcc1",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyconnectors-0.13.0-windows-2022-x86_64.zip",
            sha256: "8f2afac5b4f7a104af6bd243ee19e5dd79db6548d6420f462f11ce31b11fd8ad",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyconnectors-0.13.0-windows-11-arm64.zip",
            sha256: "0cb3d6ffbf71e7d45392072a4d1afce43a091eda27632d5b507b335fcafa2de8",
        },
    ],
    // Lazy: a user with no connected accounts should not pay to load it, and
    // most sessions never touch a connector. Safe even signed out — the module
    // loads without configuration and still answers the capability members.
    load: LoadPolicy::Lazy,
};
