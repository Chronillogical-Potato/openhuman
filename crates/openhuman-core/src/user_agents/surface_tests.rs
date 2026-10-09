use super::*;

#[test]
fn single_user_processes_are_not_narrowed() {
    let unlisted = "openhuman.config_update_autonomy_settings";
    assert!(
        visible_in(false, Scope::Operator, unlisted, false),
        "single-user core"
    );
    assert!(
        visible_in(false, Scope::User, unlisted, false),
        "embedded agent"
    );
}

#[test]
fn the_saas_planes_never_overlap() {
    let provision = "openhuman.user_agents_provision";
    let threads = "openhuman.threads_list";
    assert!(
        visible_in(true, Scope::Operator, provision, true),
        "operator reaches its plane"
    );
    assert!(
        !visible_in(true, Scope::Operator, threads, false),
        "operator never serves user methods"
    );
    assert!(
        visible_in(true, Scope::User, threads, false),
        "user reaches the allowlist"
    );
    assert!(
        !visible_in(true, Scope::User, provision, true),
        "user never reaches the operator plane"
    );
    assert!(!visible_in(
        true,
        Scope::User,
        "openhuman.config_get_config",
        false
    ));
}

#[test]
fn chat_is_open_but_other_channel_methods_are_not() {
    assert!(visible_in(
        true,
        Scope::User,
        "openhuman.channel_web_chat",
        false
    ));
    assert!(visible_in(
        true,
        Scope::User,
        "openhuman.threads_regenerate",
        false
    ));
    assert!(visible_in(
        true,
        Scope::User,
        "openhuman.memory_recall",
        false
    ));
    for method in [
        "openhuman.memory_engine_set",
        "openhuman.memory_policy_set",
        "openhuman.memory_sources_add",
        "openhuman.memory_import_start",
        "openhuman.channels_list",
        "openhuman.channels_connect",
        "openhuman.config_update_autonomy_settings",
    ] {
        assert!(!visible_in(true, Scope::User, method, false), "{method}");
    }
}

#[test]
fn every_listed_method_is_registered() {
    let registered: std::collections::HashSet<String> =
        crate::core::all::all_registered_controllers()
            .iter()
            .map(|c| c.rpc_method_name())
            .collect();
    for method in USER_METHODS {
        assert!(
            registered.contains(*method),
            "{method} is not a registered method"
        );
    }
}

#[test]
fn user_thread_ids() {
    for ok in ["thread-1", "abc_DEF-9", &"x".repeat(128)] {
        validate_user_thread_id(ok).unwrap();
    }
    for bad in [
        "",
        "channel:telegram/1",
        "proactive:job",
        "subagent:x",
        "a/b",
        "a:b",
        "../x",
        &"x".repeat(129),
    ] {
        assert!(validate_user_thread_id(bad).is_err(), "{bad:?}");
    }
}

#[test]
fn a_saas_task_without_scope_sees_nothing() {
    for (method, operator) in [
        ("openhuman.user_agents_provision", true),
        ("openhuman.threads_list", false),
    ] {
        assert!(!visible_in(true, Scope::None, method, operator), "{method}");
        assert!(visible_in(false, Scope::None, method, operator), "{method}");
    }
}

#[test]
fn delete_and_purge_stay_closed_to_users() {
    for method in ["openhuman.threads_delete", "openhuman.threads_purge"] {
        assert!(!visible_in(true, Scope::User, method, false), "{method}");
    }
}
