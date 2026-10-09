use super::*;

#[test]
fn defaults_to_build_mode_for_an_unseen_thread() {
    let thread_id = format!("run-mode-test-{}", uuid::Uuid::new_v4());
    assert_eq!(get_mode(&thread_id), RunMode::Build);
}

#[test]
fn set_mode_is_observed_through_a_fresh_handle_lookup() {
    let thread_id = format!("run-mode-test-{}", uuid::Uuid::new_v4());
    set_mode(&thread_id, RunMode::Plan);
    assert_eq!(get_mode(&thread_id), RunMode::Plan);
    set_mode(&thread_id, RunMode::Build);
    assert_eq!(get_mode(&thread_id), RunMode::Build);
}

#[test]
fn handle_for_thread_shares_state_with_set_mode() {
    let thread_id = format!("run-mode-test-{}", uuid::Uuid::new_v4());
    let handle = handle_for_thread(&thread_id);
    set_mode(&thread_id, RunMode::Plan);
    assert_eq!(handle.get(), RunMode::Plan);
}

#[test]
fn mode_label_round_trips() {
    assert_eq!(mode_label(RunMode::Plan), "plan");
    assert_eq!(mode_label(RunMode::Build), "build");
    assert_eq!(parse_mode_label("plan"), Some(RunMode::Plan));
    assert_eq!(parse_mode_label("build"), Some(RunMode::Build));
    assert_eq!(parse_mode_label("nonsense"), None);
}
