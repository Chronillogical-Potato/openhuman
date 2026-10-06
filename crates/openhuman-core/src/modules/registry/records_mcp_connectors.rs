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
    version: "0.3.7",
    release_url: "https://github.com/tinyhumansai/tinymcp/releases/tag/v0.3.7",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinymcp-0.3.7-ubuntu-24.04-x86_64.tar.gz",
            sha256: "ead6a803100ebb5cecd581b585be33022b0d6c26c52336193268e1630c7f059e",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinymcp-0.3.7-ubuntu-24.04-arm64.tar.gz",
            sha256: "80c5ed67019c8f8a36bcbbeb757e12dfddd9a3afeea108d0678be1638c13b0de",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinymcp-0.3.7-ubuntu-22.04-x86_64.tar.gz",
            sha256: "eb6b868b6fc691d2c77e32ae91f122f020c27a55dd59ad7020ebffdf98c186fe",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinymcp-0.3.7-ubuntu-22.04-arm64.tar.gz",
            sha256: "19e5d819372766c67f1b20338b2e772a2e3e3c287af976c9ec24eb50a899afff",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinymcp-0.3.7-macos-26-arm64.tar.gz",
            sha256: "9543db96e4246adec0b5f6dc626e38d0ea061fde1f22d69fc9302da043b78565",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinymcp-0.3.7-macos-26-x86_64.tar.gz",
            sha256: "de95173aa31dd3cabb4f15d6395f163feed8a9c8b41c3ac961d29848b4f82ebb",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinymcp-0.3.7-macos-15-arm64.tar.gz",
            sha256: "8b202d904a2ce28e35a91910d440b5a6acc289f3cc8b46f4e9fdea83c06f95e0",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinymcp-0.3.7-macos-15-x86_64.tar.gz",
            sha256: "3866853bca6958fd48e9d1386de914bfb639b3fc0c414cf6f921f2d02fcb4ac9",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinymcp-0.3.7-windows-2025-x86_64.zip",
            sha256: "3dcfd4c55fba18303ead905b0193caa36bae6fc21696a58d034f2434df536e6b",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinymcp-0.3.7-windows-2022-x86_64.zip",
            sha256: "5c8d9b090f45710180bff6e890328c650138149dd4bf7076249b52464055a593",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinymcp-0.3.7-windows-11-arm64.zip",
            sha256: "4b23cc86a02396c9804cd7dfcda2367b18d7ecd539fe2cc6411fca1dac41cfda",
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
