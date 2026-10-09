//! `host::tui()` builds a connected in-process runtime and a controller call
//! works through it. Its own test binary: a runtime claims a process-wide slot.
#![cfg(feature = "session-store")]

#[test]
fn tui_host_builds_a_runtime_that_answers_a_read_rpc() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    std::env::set_var("OPENHUMAN_WORKSPACE", workspace.path());

    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(16 * 1024 * 1024)
                .build()
                .expect("tokio runtime")
                .block_on(async {
                    let runtime = openhuman_rpc::host::tui()
                        .await
                        .expect("tui host builds a runtime");
                    let listed = runtime
                        .core_runtime()
                        .invoke("openhuman.threads_list", serde_json::json!({}))
                        .await
                        .expect("threads_list answers in-process");
                    assert!(listed.is_object() || listed.is_array(), "{listed}");
                    drop(runtime);
                });
        })
        .expect("test thread")
        .join()
        .expect("test thread should not panic");
}
