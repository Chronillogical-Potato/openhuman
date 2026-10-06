//! Registry record for the `tinyvoice` module.

use crate::modules::types::{LoadPolicy, ModuleRecord, PlatformAsset};

/// The `tinyvoice` module: the host-agnostic half of the voice pipeline.
///
/// Wake-word gating, fast-path command routing, STT hallucination detection,
/// and the capture-side audio work (downmix, resample, silence gate, WAV
/// framing).
///
/// Lazy, and more clearly so than the others: voice is opt-in twice over — a
/// user has to enable dictation or always-on listening before any of this runs
/// — so a session that never speaks should not pay a download or a `dlopen`.
///
/// **The VAD deliberately does not come through here.** A segmenter is driven
/// once per 20 ms frame from inside a `cpal` callback, and a bus round trip at
/// that cadence would cost more than the sixty-line state machine it replaces.
/// `voice::always_on` keeps its own; see [`super::voice`].
pub(crate) const TINYVOICE: ModuleRecord = ModuleRecord {
    id: "tinyvoice",
    description: "Wake-word gating, command routing, hallucination detection, capture audio",
    bus_name: "ai.tinyhumans.tinyvoice.Voice",
    object_path: "/ai/tinyhumans/tinyvoice/Voice",
    version: "0.1.11",
    release_url: "https://github.com/tinyhumansai/tinyvoice/releases/tag/v0.1.11",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyvoice-module-0.1.11-ubuntu-24.04-x86_64.tar.gz",
            sha256: "f46e03979247b2a6d806c11419edf5aa9980ef88ae35332442496a26ef2b7e19",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyvoice-module-0.1.11-ubuntu-24.04-arm64.tar.gz",
            sha256: "d32bfb9dea4bb4bc248ca3ccc9adc7f9f101b9b44f80b67b63b966e95af23e49",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyvoice-module-0.1.11-ubuntu-22.04-x86_64.tar.gz",
            sha256: "8f8b00f3302aa8257d483702e2fb7e6e15e1123ebfc3581e88bc244e3d4e1aa6",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyvoice-module-0.1.11-ubuntu-22.04-arm64.tar.gz",
            sha256: "e83e54d4db292fc96ef8aaeb6903212015d3ba4933a7fe5ea3b183179d26b8bf",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyvoice-module-0.1.11-macos-26-arm64.tar.gz",
            sha256: "ec764c3880bb817410033fe103511fb0e50a6c3f3ee75baaec634c2d7c192d8d",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyvoice-module-0.1.11-macos-26-x86_64.tar.gz",
            sha256: "fe4d055d8138e94cea3c4eecb9695ab44a652e84cd39491ad923bf97891de5b3",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyvoice-module-0.1.11-macos-15-arm64.tar.gz",
            sha256: "556bb4e8777a7d4b17221cf96925c60ba317236a455001c361d4d7bb11933568",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyvoice-module-0.1.11-macos-15-x86_64.tar.gz",
            sha256: "961fe70889a6bd3f699b2338ad3cbe5fc5ae347fe290c65a70bd438b1a95c16d",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyvoice-module-0.1.11-windows-2025-x86_64.zip",
            sha256: "4876c90f443d07c2ef57da4360c07ac3f12352c8880ca9ab63e3488538486a55",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyvoice-module-0.1.11-windows-2022-x86_64.zip",
            sha256: "c0e1204efa63fb2c72ab4076e8bb805eed65ec5b964bb48fbe53f97686b75f66",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyvoice-module-0.1.11-windows-11-arm64.zip",
            sha256: "895ca2bf55c71dab2e72cc0025a3f07bd25997e4ed4c9125f5d0cdaa09906986",
        },
    ],
    load: LoadPolicy::Lazy,
};
