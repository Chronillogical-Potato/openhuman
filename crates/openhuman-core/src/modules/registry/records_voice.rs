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
    version: "0.1.7",
    release_url: "https://github.com/tinyhumansai/tinyvoice/releases/tag/v0.1.7",
    assets: &[
        PlatformAsset {
            host_key: "ubuntu-24.04-x86_64",
            archive: "tinyvoice-module-0.1.7-ubuntu-24.04-x86_64.tar.gz",
            sha256: "491a3f01b53a671caa9223e9f34da77274eb64495765418bba9764838063e36a",
        },
        PlatformAsset {
            host_key: "ubuntu-24.04-arm64",
            archive: "tinyvoice-module-0.1.7-ubuntu-24.04-arm64.tar.gz",
            sha256: "f81ccb094882dc8175752ca8b16b47f9bae1dfd533de913d77fec9091237dc88",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-x86_64",
            archive: "tinyvoice-module-0.1.7-ubuntu-22.04-x86_64.tar.gz",
            sha256: "c07ae2ff370b14ad714af25a6df0dc3cb053b7aa930311c725e48919e9828794",
        },
        PlatformAsset {
            host_key: "ubuntu-22.04-arm64",
            archive: "tinyvoice-module-0.1.7-ubuntu-22.04-arm64.tar.gz",
            sha256: "866f3d688fb71d0e78e469018619dbb9f36754757b89a964b4bdb64e03271368",
        },
        PlatformAsset {
            host_key: "macos-26-arm64",
            archive: "tinyvoice-module-0.1.7-macos-26-arm64.tar.gz",
            sha256: "92c8d0b87c218706f9e7cc91e9639dfab2fea3b5888e1095f67bc1e3d0176b06",
        },
        PlatformAsset {
            host_key: "macos-26-x86_64",
            archive: "tinyvoice-module-0.1.7-macos-26-x86_64.tar.gz",
            sha256: "1d8bd33a797eb5896249bd179ad0015c16a8c2f0aacb2d0d4651ca0ca73169fb",
        },
        PlatformAsset {
            host_key: "macos-15-arm64",
            archive: "tinyvoice-module-0.1.7-macos-15-arm64.tar.gz",
            sha256: "2925e5aacea43a050a65931c56e605d93fe0bbcb79118e03abb38a4fdd370924",
        },
        PlatformAsset {
            host_key: "macos-15-x86_64",
            archive: "tinyvoice-module-0.1.7-macos-15-x86_64.tar.gz",
            sha256: "ae4997a13fd3daf6c625cefe6dac271726de66226b9359e50e28b02e19968dc3",
        },
        PlatformAsset {
            host_key: "windows-2025-x86_64",
            archive: "tinyvoice-module-0.1.7-windows-2025-x86_64.zip",
            sha256: "9f8f4a4074241659f866710e1d3f00d9ea7ae4d9790de975dd3a96be2a035095",
        },
        PlatformAsset {
            host_key: "windows-2022-x86_64",
            archive: "tinyvoice-module-0.1.7-windows-2022-x86_64.zip",
            sha256: "d92fd9d73758b509fe79ccc29814608e18a5a67cbcc839fe635752ad64e29721",
        },
        PlatformAsset {
            host_key: "windows-11-arm64",
            archive: "tinyvoice-module-0.1.7-windows-11-arm64.zip",
            sha256: "dd901a4e164125e561054fb750c444db5e4915d8c63b8cf0429ebc289ff46994",
        },
    ],
    load: LoadPolicy::Lazy,
};
