use super::VOICE_COMPILED_IN;

/// Pins the constant to the gate rather than to a hardcoded value: the
/// assertion inverts with the feature, so it holds for both the default
/// build and the slim (`--no-default-features`) build.
#[test]
fn reports_the_compiled_gate_state() {
    assert_eq!(VOICE_COMPILED_IN, cfg!(feature = "voice"));
}
