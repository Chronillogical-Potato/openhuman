use std::ffi::OsString;

use crate::server::testing::EnvVarGuard;

#[test]
fn e2e_environment_enables_advertised_tool_groups() {
    let _guard = EnvVarGuard::set_many(vec![("OPENHUMAN_E2E", "1".into())]);
    let builder =
        openhuman_core::core::runtime::CoreBuilder::new(openhuman_core::core::types::HostKind::Cli);
    let _ = super::apply_e2e_tool_groups(builder);
}

#[test]
fn core_listener_settings_use_valid_environment_values_and_safe_defaults() {
    {
        let _guard = EnvVarGuard::set_many(vec![
            ("OPENHUMAN_CORE_PORT", "8123".into()),
            ("OPENHUMAN_CORE_HOST", "0.0.0.0".into()),
        ]);
        assert_eq!(super::core_port(), 8123);
        assert_eq!(super::core_host(), "0.0.0.0");
    }

    let _invalid = EnvVarGuard::set_many(vec![
        ("OPENHUMAN_CORE_PORT", "not-a-port".into()),
        ("OPENHUMAN_CORE_HOST", "".into()),
    ]);
    assert_eq!(super::core_port(), 7788);
    assert_eq!(super::core_host(), "127.0.0.1");
}

#[test]
fn server_shim_refuses_public_bind_without_operator_token() {
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("tokio runtime")
                .block_on(async {
                    let workspace = tempfile::tempdir().expect("workspace tempdir");
                    let _env = EnvVarGuard::set_many(vec![
                        (
                            "OPENHUMAN_WORKSPACE",
                            workspace.path().as_os_str().to_os_string(),
                        ),
                        ("OPENHUMAN_CORE_TOKEN", OsString::from("")),
                    ]);
                    let services = openhuman_core::core::runtime::ServiceSet::headless_api();

                    let error = super::run_server_with_services(
                        Some("0.0.0.0"),
                        Some(0),
                        services,
                        true,
                        None,
                        None,
                        None,
                    )
                    .await
                    .expect_err("public bind must require an operator-supplied token");

                    assert!(error
                        .to_string()
                        .contains("refusing to bind on non-loopback"));
                });
        })
        .expect("test thread")
        .join()
        .expect("test thread should not panic");
}
