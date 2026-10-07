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
    description: "OAuth connector integrations: accounts, actions, triggers, and record sync",
    bus_name: "ai.tinyhumans.connectors.Composio",
    object_path: "/ai/tinyhumans/connectors/Composio",
    version: "0.12.3",
    release_url: "https://github.com/tinyhumansai/tinyconnectors/releases/tag/v0.12.3",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyconnectors-0.12.3-ubuntu-24.04-x86_64.tar.gz",
            sha256: "6cd73e820d751a3e02270836df93dd0f12fe5c1e7160f2cc3dd12bf413d4aafa",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyconnectors-0.12.3-ubuntu-24.04-arm64.tar.gz",
            sha256: "6c2144408587b3d5a84369c9c64a658c9de87adb1065c19970de05fdffb2ebd4",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyconnectors-0.12.3-ubuntu-22.04-x86_64.tar.gz",
            sha256: "87e59ab0e1c8cba3438f7b409e9387bd36ab18f4f410ee2a7dd046d496ce515d",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyconnectors-0.12.3-ubuntu-22.04-arm64.tar.gz",
            sha256: "06db0bef57c48d87a1aaaa94413799e1346bc42ff5faaa513b87c8f1e19ca3c2",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyconnectors-0.12.3-macos-26-arm64.tar.gz",
            sha256: "fe82e869c9b0b4404c3226f6c71e4564c3a3c37b811e1288eda3cb0040434598",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyconnectors-0.12.3-macos-26-x86_64.tar.gz",
            sha256: "3435dbf8afc3e41ae4dec4cad1543d160bbd2a9eb8d53a946e5e7830333acfc0",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyconnectors-0.12.3-macos-15-arm64.tar.gz",
            sha256: "5ce44797e8a124221f6f858819b2f2d4be2602b65396ee5050688d77e11f353d",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyconnectors-0.12.3-macos-15-x86_64.tar.gz",
            sha256: "4c294af196b2ab8d1bd09c19e5df6bc2318be6fcb4b1de2f69b5483dc5087c9a",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyconnectors-0.12.3-windows-2025-x86_64.zip",
            sha256: "6195ac96091b2128a1d4f83d5b49840d3908d3684822d1095c7cd417743b0f2d",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyconnectors-0.12.3-windows-2022-x86_64.zip",
            sha256: "937cdda361805bc61ec5fe0e5fe649a6e834da68c7318c174735c4d834c01c6e",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyconnectors-0.12.3-windows-11-arm64.zip",
            sha256: "9b1f24d8727d0a184662e5045fcda9c604b3b48834350b2c9569c66cca8130c7",
        },
    ],
    // Lazy: a user with no connected accounts should not pay to load it, and
    // most sessions never touch a connector. Safe even signed out — the module
    // loads without configuration and still answers the capability members.
    load: LoadPolicy::Lazy,
};
