use super::*;

#[test]
fn flag_values_parse() {
    assert_eq!(mode_from(None), Mode::Off);
    assert_eq!(mode_from(Some("off")), Mode::Off);
    assert_eq!(mode_from(Some(" ON ")), Mode::On);
    assert_eq!(mode_from(Some("1")), Mode::On);
    assert_eq!(mode_from(Some("ab")), Mode::Ab { percent: 50 });
    assert_eq!(mode_from(Some("ab:20")), Mode::Ab { percent: 20 });
    assert_eq!(mode_from(Some("ab:300")), Mode::Ab { percent: 100 });
    assert_eq!(mode_from(Some("ab:99999")), Mode::Ab { percent: 100 });
    assert_eq!(mode_from(Some("ab:-5")), Mode::Off);
    assert_eq!(mode_from(Some("ab:250")), Mode::Ab { percent: 100 });
    assert_eq!(mode_from(Some("ab:100")), Mode::Ab { percent: 100 });
    assert_eq!(mode_from(Some("sometimes")), Mode::Off);
}

#[test]
fn assignment_is_stable_and_respects_the_split() {
    assert_eq!(assign(Mode::Off, "t"), Arm::Disabled);
    assert_eq!(assign(Mode::On, "t"), Arm::Treatment);
    assert_eq!(assign(Mode::Ab { percent: 0 }, "t"), Arm::Control);
    assert_eq!(assign(Mode::Ab { percent: 100 }, "t"), Arm::Treatment);

    let a = assign(Mode::Ab { percent: 50 }, "thread-abc");
    for _ in 0..10 {
        assert_eq!(assign(Mode::Ab { percent: 50 }, "thread-abc"), a);
    }

    let treated = (0..2000)
        .filter(|i| assign(Mode::Ab { percent: 30 }, &format!("thread-{i}")) == Arm::Treatment)
        .count();
    assert!((450..750).contains(&treated), "treated={treated}");
}

#[test]
fn bucket_is_fixed_across_builds() {
    // FNV-1a is specified, so this value must never change; a change would
    // silently move live threads between arms.
    assert_eq!(bucket("thread-abc"), bucket("thread-abc"));
    assert_eq!(bucket(""), (0xcbf2_9ce4_8422_2325_u64 % 100) as u8);
}

#[test]
fn runtime_flag_overrides_the_build_default() {
    assert_eq!(resolve_mode(None, None), Mode::Off);
    assert_eq!(resolve_mode(None, Some("on")), Mode::On);
    assert_eq!(resolve_mode(None, Some("ab:30")), Mode::Ab { percent: 30 });
    // A deployment can always switch a shipped default back off.
    assert_eq!(resolve_mode(Some("off"), Some("on")), Mode::Off);
    assert_eq!(
        resolve_mode(Some("ab"), Some("on")),
        Mode::Ab { percent: 50 }
    );
}
