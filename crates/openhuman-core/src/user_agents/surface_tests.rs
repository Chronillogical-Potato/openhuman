use super::*;

#[test]
fn single_user_processes_are_not_narrowed() {
    let unlisted = "openhuman.config_update_autonomy_settings";
    assert!(
        visible_in(false, false, unlisted, false),
        "single-user core"
    );
    assert!(visible_in(false, true, unlisted, false), "embedded agent");
}

#[test]
fn the_saas_planes_never_overlap() {
    let provision = "openhuman.user_agents_provision";
    let threads = "openhuman.threads_list";
    assert!(
        visible_in(true, false, provision, true),
        "operator reaches its plane"
    );
    assert!(
        !visible_in(true, false, threads, false),
        "operator never serves user methods"
    );
    assert!(
        visible_in(true, true, threads, false),
        "user reaches the allowlist"
    );
    assert!(
        !visible_in(true, true, provision, true),
        "user never reaches the operator plane"
    );
    assert!(!visible_in(
        true,
        true,
        "openhuman.config_get_config",
        false
    ));
}

#[test]
fn chat_is_open_but_other_channel_methods_are_not() {
    assert!(visible_in(true, true, "openhuman.channel_web_chat", false));
    assert!(visible_in(
        true,
        true,
        "openhuman.threads_regenerate",
        false
    ));
    assert!(visible_in(true, true, "openhuman.memory_recall", false));
    for method in [
        "openhuman.memory_engine_set",
        "openhuman.memory_policy_set",
        "openhuman.memory_sources_add",
        "openhuman.memory_import_start",
        "openhuman.channels_list",
        "openhuman.channels_connect",
        "openhuman.config_update_autonomy_settings",
    ] {
        assert!(!visible_in(true, true, method, false), "{method}");
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
