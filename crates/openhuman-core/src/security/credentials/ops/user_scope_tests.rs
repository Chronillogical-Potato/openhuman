use super::*;

#[test]
fn explicit_scratch_workspace_cannot_activate_operator_user() {
    let _guard = crate::config::TEST_ENV_LOCK.lock().unwrap();
    let previous = std::env::var_os("OPENHUMAN_WORKSPACE");
    let scratch = tempfile::tempdir().unwrap();
    unsafe {
        std::env::set_var("OPENHUMAN_WORKSPACE", scratch.path());
    }
    assert!(!operator_user_activation_allowed());
    match previous {
        Some(value) => unsafe { std::env::set_var("OPENHUMAN_WORKSPACE", value) },
        None => unsafe { std::env::remove_var("OPENHUMAN_WORKSPACE") },
    }
}
