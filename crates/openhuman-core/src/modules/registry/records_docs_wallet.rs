//! Registry records for the `tinydocs` and `tinywallet` modules.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The `tinydocs` module: `.docx` / `.pptx` synthesis and `.pdf` extraction.
///
/// Lazy, because a user who never asks for a document should not pay a download,
/// a `dlopen`, and the resident cost of a library that is never unloaded.
pub(crate) const TINYDOCS: ModuleRecord = ModuleRecord {
    id: "tinydocs",
    description: "Document synthesis (.docx, .pptx) and PDF text extraction",
    bus_name: "ai.tinyhumans.tinydocs.Documents",
    object_path: "/ai/tinyhumans/tinydocs/Documents",
    version: "0.1.16",
    release_url: "https://github.com/tinyhumansai/tinydocs/releases/tag/v0.1.16",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinydocs-module-0.1.16-ubuntu-24.04-x86_64.tar.gz",
            sha256: "2cfa9e5480722166e12868b017e8b42bdbb12df04d20b592c9307b5082c21d58",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinydocs-module-0.1.16-ubuntu-24.04-arm64.tar.gz",
            sha256: "8d1ab2eb13d7e0b4a386dafe3191018df11ba457565842db358ec56f77a763a1",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinydocs-module-0.1.16-ubuntu-22.04-x86_64.tar.gz",
            sha256: "48a73a0893dc31405c59022f9e7024edcc55546462656b414c067b14e87074b1",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinydocs-module-0.1.16-ubuntu-22.04-arm64.tar.gz",
            sha256: "0bec682efcc6876e1025f77d632821cb879557148009bff3f45f5e4c8a7ff54f",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinydocs-module-0.1.16-macos-26-arm64.tar.gz",
            sha256: "2ccb88171233a039463d0c7c5ed89842a8092505835a8848afbcda79c492510e",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinydocs-module-0.1.16-macos-26-x86_64.tar.gz",
            sha256: "158c30301017ceaf83dc70118bf36ca736118817b02dd5f797a2327398109346",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinydocs-module-0.1.16-macos-15-arm64.tar.gz",
            sha256: "451ec94e25c66bf0902e972847308a955d04f2506ec22501fb256e4a930ce3e9",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinydocs-module-0.1.16-macos-15-x86_64.tar.gz",
            sha256: "9b17eb8886bba09d3cd9ff23fc24fd0f46c7bb55d8b091e4853892d3de27591d",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinydocs-module-0.1.16-windows-2025-x86_64.zip",
            sha256: "a75c06786f60f598627245aa9fbeb2840e28170417b0415a5bd82332ead5ac03",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinydocs-module-0.1.16-windows-2022-x86_64.zip",
            sha256: "8f9855184738af9de9bd0b1728fa80e852ae91667f5e099b6c6c1096b9c7e132",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinydocs-module-0.1.16-windows-11-arm64.zip",
            sha256: "ab574f056dbb09fdb4e6d5c0f848591fb3a59f6872e0f012bbb66500819ee9b8",
        },
    ],
    load: LoadPolicy::Lazy,
};

/// The `tinywallet` module: transaction building and assembly for four chains.
///
/// Lazy for the same reason as [`TINYDOCS`], and more so: most sessions never
/// touch a wallet, and this artifact carries `bitcoin` and a native `secp256k1`
/// build that would otherwise be resident for all of them.
///
/// **This host sends it the recovery phrase, over confidential calls, and never
/// derives or signs itself.** All four chains — Bitcoin, EVM, Solana and Tron —
/// derive and sign inside the module. This binary does not link the root
/// `tinywallet` crate at all — it takes `tinywallet-bus`, the wire contract,
/// which carries no `key` gate — nor does it link `k256`; see the note on the
/// `tinywallet-bus` dependency.
///
/// The phrase is only sent to a module tinybus has attested *and* whose digest
/// matches one of the entries below — `super::wallet::attested_proxy` checks
/// this table itself rather than trusting that some check happened.
///
/// The contract also exposes `ExportKey` for downstream hosts that must drive
/// a signer locally; OpenHuman itself does not call it.
///
/// Three releases got here, and the order mattered. v0.2.3 changed no method at
/// all — it was the same module rebuilt against a bus that could attest it.
/// Attestation used to be recorded only from a `modules.toml` beside the
/// artifact, and a release download extracts into a temporary directory that has
/// none, so this module could never be an attested recipient however carefully
/// the digest below was pinned (tinybus#15 fixed that). Only then was it safe
/// for v0.3.0 to add methods that take a secret, and for v0.4.0 to add
/// `SignMessage` for the Solana and x402 encodings the wire contract does not
/// model. Adding them earlier would have made them unreachable in production and
/// reachable in a developer's tree, which is the worst of both.
pub(crate) const TINYWALLET: ModuleRecord = ModuleRecord {
    id: "tinywallet",
    description: "Transaction building and assembly for Bitcoin, EVM, Solana and Tron",
    bus_name: "ai.tinyhumans.tinywallet.Wallet",
    object_path: "/ai/tinyhumans/tinywallet/Wallet",
    version: "0.5.2",
    release_url: "https://github.com/tinyhumansai/tinywallet/releases/tag/v0.5.2",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinywallet-module-0.5.2-ubuntu-24.04-x86_64.tar.gz",
            sha256: "a18fb6e9bd7ec765ed0005954e229306e2fd4c02242d02bd61c7e3a4bc8237ca",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinywallet-module-0.5.2-ubuntu-24.04-arm64.tar.gz",
            sha256: "caf961b2fbabdf2327bea0532269dddc7acd7796a3ef0c7ea2e6d590d97eeeaf",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinywallet-module-0.5.2-ubuntu-22.04-x86_64.tar.gz",
            sha256: "1c154c7586f3786f374c6178e253ed907a0f3df396a44ca98fb43fde3c8043a5",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinywallet-module-0.5.2-ubuntu-22.04-arm64.tar.gz",
            sha256: "eede5f0e1495de99a861c823765352fc3f5e63ce95f06a33476c2e80c12d1469",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinywallet-module-0.5.2-macos-26-arm64.tar.gz",
            sha256: "490479ddc177fac17eed5a77cfb41623043b5863e8e98cdde9a2e90e49127613",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinywallet-module-0.5.2-macos-26-x86_64.tar.gz",
            sha256: "177b4ef67785fbcf0ec8c0b6cf99e1ad5129e31392ecfab90f130c39fa4db303",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinywallet-module-0.5.2-macos-15-arm64.tar.gz",
            sha256: "b359b56c573c0a97ccb18e4f360a37c9608d3724a573c98ea856c06c3e26b472",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinywallet-module-0.5.2-macos-15-x86_64.tar.gz",
            sha256: "36b6515202dbb1b20f0d60cbb6f929d0e70c7a766e47de0732fc890f7f553750",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinywallet-module-0.5.2-windows-2025-x86_64.zip",
            sha256: "ee8867c0aba30c1c133a4402f4921308906b211877a77562c69668237ac0a2c7",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinywallet-module-0.5.2-windows-2022-x86_64.zip",
            sha256: "188df566ba9b69055db65148238ddd975b5034ca57e228c9c333af834446ea9d",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinywallet-module-0.5.2-windows-11-arm64.zip",
            sha256: "a1e2c7b0c54a6f7f3a7f4b8ac99d38e0ffabe44ae43784cd5b747b1ceb082fd5",
        },
    ],
    load: LoadPolicy::Lazy,
};
