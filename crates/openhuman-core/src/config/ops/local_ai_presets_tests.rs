//! RAM-driven local-model tier selection, asserted from OpenHuman's own seam.
//!
//! # Why these live here and not upstream
//!
//! The thresholds these tests pin (`MIN_RAM_GB_FOR_LOCAL_AI`, the `recommend_tier`
//! bands) belong to `tinyinference_local::presets`, which has its own unit tests
//! in `presets_test.rs` / `presets_device_test.rs`. **No OpenHuman lane runs
//! them.** `vendor` is listed in the root `[workspace] exclude`, both Rust lanes
//! invoke `cargo test --manifest-path Cargo.toml --workspace` or a named
//! `--test <target>`, and no CI step names `tinyinference-local`. The crate sits
//! two submodule hops down, under `vendor/tinyagents/vendor/tinyinference/`.
//!
//! OpenHuman *consumes* those thresholds — `inference_apply_preset` and
//! `inference_presets` are built on `apply_preset_to_config` and
//! `current_tier_from_config` in this module, and `inference/ops.rs` calls
//! `detect_device_profile` and `presets::recommend_tier` directly. So an
//! upstream change to a band silently changes what OpenHuman recommends, and
//! nothing that runs would notice.
//!
//! These are therefore **consumer contract tests**: they assert the behaviour
//! OpenHuman depends on, at the version OpenHuman has pinned. They are not a
//! second copy of the upstream suite — they cover only the values this repo's
//! own RPC surface hands to users.
//!
//! # What is NOT covered here, and cannot be
//!
//! `inference_device_profile` reports the *real host's* RAM, so the RPC layer
//! can only assert `total_ram_bytes > 0` (which `tests/json_rpc_e2e.rs` already
//! does). OpenHuman exposes no seam that injects a synthetic device profile, so
//! the threshold behaviour is reachable only by constructing a `DeviceProfile`
//! in-process, as below. An end-to-end "this 4 GB machine defaulted to cloud"
//! assertion would need that injection seam and is not written.

use super::*;
use tinyinference_local::device::DeviceProfile;
use tinyinference_local::presets::{
    device_supports_local_ai, recommend_tier, should_default_to_cloud_fallback,
    MIN_RAM_GB_FOR_LOCAL_AI, MVP_MAX_TIER,
};

/// A synthetic host with `ram_gb` of physical memory and nothing else notable.
///
/// Every other field is fixed so a test that fails names the RAM band and not
/// an incidental difference.
fn device_with_ram_gb(ram_gb: u64) -> DeviceProfile {
    DeviceProfile {
        total_ram_bytes: ram_gb * 1024 * 1024 * 1024,
        cpu_count: 8,
        cpu_brand: "test-cpu".to_string(),
        os_name: "test-os".to_string(),
        os_version: "0.0".to_string(),
        has_gpu: false,
        gpu_description: None,
    }
}

/// Matrix 3.3.1.3 — over-allocation prevention.
///
/// A host below the floor must not be told to run local inference. The floor is
/// read from the constant rather than hard-coded to 8, so a deliberate upstream
/// move changes one number here and an accidental one still fails the band
/// assertions below.
#[test]
fn a_host_below_the_ram_floor_is_not_offered_local_inference() {
    let below = device_with_ram_gb(MIN_RAM_GB_FOR_LOCAL_AI - 1);

    assert!(
        !device_supports_local_ai(&below),
        "a {} GB host is below the {MIN_RAM_GB_FOR_LOCAL_AI} GB floor and must not \
         be offered local inference by default",
        below.total_ram_gb()
    );
    assert!(
        should_default_to_cloud_fallback(&below),
        "below the floor the recommendation must be cloud fallback"
    );
}

/// Matrix 3.3.1.4 — under-allocation handling.
///
/// The mirror of the case above: a host *at* the floor must be offered local
/// inference, and must not be pushed to cloud. Written as a separate test so a
/// regression that refuses everything fails here and passes above, rather than
/// both assertions agreeing inside one test that only proves the guard is
/// reachable.
#[test]
fn a_host_at_the_ram_floor_is_offered_local_inference() {
    let at_floor = device_with_ram_gb(MIN_RAM_GB_FOR_LOCAL_AI);

    assert!(
        device_supports_local_ai(&at_floor),
        "the floor is inclusive: a {MIN_RAM_GB_FOR_LOCAL_AI} GB host qualifies"
    );
    assert!(
        !should_default_to_cloud_fallback(&at_floor),
        "at or above the floor the recommendation must not be cloud fallback"
    );
}

/// Matrix 3.3.2.2 — model switching based on memory.
///
/// Pins every band of `recommend_tier`, including both sides of each boundary.
/// A band that silently widens or shifts by one gigabyte changes which model a
/// user's machine downloads, which is not something a smoke test would catch.
#[test]
fn recommended_tier_tracks_host_ram_at_every_band_boundary() {
    let cases: &[(u64, ModelTier)] = &[
        (0, ModelTier::Ram1Gb),
        (1, ModelTier::Ram1Gb),
        (2, ModelTier::Ram2To4Gb),
        (3, ModelTier::Ram2To4Gb),
        (4, ModelTier::Ram4To8Gb),
        (7, ModelTier::Ram4To8Gb),
        (8, ModelTier::Ram8To16Gb),
        (15, ModelTier::Ram8To16Gb),
        (16, ModelTier::Ram16PlusGb),
        (128, ModelTier::Ram16PlusGb),
    ];

    for (ram_gb, expected) in cases {
        let actual = recommend_tier(&device_with_ram_gb(*ram_gb));
        assert_eq!(
            actual, *expected,
            "a {ram_gb} GB host should be recommended {expected:?}, got {actual:?}"
        );
    }
}

/// The MVP ceiling is a product decision, not an accident of the band table.
///
/// `MVP_MAX_TIER` blocks larger local models "to keep summarization lightweight
/// and battery-friendly", so a 128 GB workstation is still *recommended*
/// `Ram16PlusGb` while only `Ram2To4Gb` is `is_mvp_allowed`. Those two facts
/// disagreeing is the shape of a bug, so pin both together: the recommendation
/// is about the hardware, the ceiling is about what ships.
#[test]
fn the_mvp_ceiling_is_independent_of_what_the_hardware_can_run() {
    let workstation = device_with_ram_gb(128);

    assert_eq!(recommend_tier(&workstation), ModelTier::Ram16PlusGb);
    assert!(
        !ModelTier::Ram16PlusGb.is_mvp_allowed(),
        "the recommendation is not itself an allow-list entry"
    );
    assert!(
        MVP_MAX_TIER.is_mvp_allowed(),
        "the declared MVP ceiling must be allowed by the predicate that enforces it"
    );
}

/// Matrix 3.3.3.1 — saving a RAM-tier selection, through OpenHuman's own seam.
///
/// `inference_apply_preset` is `apply_preset_to_config`, and `inference_presets`
/// reports `current_tier_from_config`. The RPC round-trip is covered in
/// `tests/json_rpc_e2e.rs`; what is asserted here is the part that RPC test
/// cannot see — that the config fields the preset writes are the ones
/// `current_tier_from_config` reads back, for *every* real tier rather than the
/// single `ram_2_4gb` the RPC test exercises.
#[test]
fn applying_a_preset_round_trips_through_the_config_for_every_real_tier() {
    for tier in [
        ModelTier::Ram1Gb,
        ModelTier::Ram2To4Gb,
        ModelTier::Ram4To8Gb,
        ModelTier::Ram8To16Gb,
        ModelTier::Ram16PlusGb,
    ] {
        let mut config = LocalAiConfig::default();
        apply_preset_to_config(&mut config, tier);

        assert_eq!(
            config.selected_tier.as_deref(),
            Some(tier.as_str()),
            "applying {tier:?} must record its canonical id"
        );
        assert!(
            config.runtime_enabled,
            "applying {tier:?} must enable the local runtime"
        );
        assert_eq!(
            current_tier_from_config(&config),
            tier,
            "{tier:?} must read back as itself; if it does not, the preset writes \
             fields that preset_matches_config does not compare"
        );
    }
}

/// `Custom` is not a preset, and applying it must not silently rewrite the
/// user's model choices.
///
/// `apply_preset_to_config` no-ops for `Custom` (`preset_for_tier` returns
/// `None`). Without this, a future refactor that gave `Custom` a preset row
/// would clobber a hand-configured local setup on any code path that applies
/// the "current" tier back.
#[test]
fn applying_the_custom_tier_leaves_a_hand_configured_setup_untouched() {
    let mut config = LocalAiConfig::default();
    config.chat_model_id = "my-own-model".to_string();
    config.vision_model_id = "my-own-vision".to_string();
    let before = config.clone();

    apply_preset_to_config(&mut config, ModelTier::Custom);

    assert_eq!(
        config.chat_model_id, before.chat_model_id,
        "applying Custom must not rewrite the chat model"
    );
    assert_eq!(
        config.vision_model_id, before.vision_model_id,
        "applying Custom must not rewrite the vision model"
    );
    assert_eq!(
        current_tier_from_config(&config),
        ModelTier::Custom,
        "a config matching no preset reads back as Custom"
    );
}

/// Matrix 3.3.3.3 — returning to the default.
///
/// There is no "reset to default" control in the product (see `e2e-gaps-w4.md`);
/// what exists is `LocalAiConfig::default()`, which is the state a fresh install
/// and a full data reset both land on.
///
/// The non-obvious part, and the reason this is worth pinning: **a fresh config
/// already sits on a real preset tier it was never given.** `selected_tier` is
/// `None`, but the defaults (`gemma3:1b-it-qat`, no vision model, `bge-m3`) are
/// byte-identical to the `Ram2To4Gb` preset, so `current_tier_from_config` falls
/// through its `all_presets()` scan and reports `Ram2To4Gb` — which is also
/// `MVP_MAX_TIER`. A "reset to default" built on the assumption that a fresh
/// config is `Custom` would therefore be wrong, and so was this test's first
/// draft.
///
/// If a default model id changes upstream without the matching preset changing,
/// this flips to `Custom` and every new install silently stops matching a
/// preset. That is the regression this catches.
#[test]
fn the_default_local_ai_config_already_matches_the_mvp_preset_tier() {
    let config = LocalAiConfig::default();

    assert!(
        config.selected_tier.is_none(),
        "a fresh config must not claim a tier it was never given"
    );
    assert_eq!(
        current_tier_from_config(&config),
        ModelTier::Ram2To4Gb,
        "the default model ids are the Ram2To4Gb preset, so a fresh config \
         resolves to that tier despite selected_tier being None"
    );
    assert_eq!(
        current_tier_from_config(&config),
        MVP_MAX_TIER,
        "the tier a fresh install lands on must be the one the MVP ceiling allows"
    );
    assert!(
        !supports_screen_summary(&config),
        "the default preset disables vision, so no screen summary"
    );
}
