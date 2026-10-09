//! `host::cli` dispatches a subcommand in-process and returns. Its own test
//! binary: the CLI boot installs process-lifetime seams that are never restored.
#![cfg(feature = "server")]

#[test]
fn cli_host_runs_help_and_returns_ok() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    std::env::set_var("OPENHUMAN_WORKSPACE", workspace.path());

    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            openhuman_rpc::host::cli(&["--help".to_string()]).expect("--help dispatches and exits Ok");
            openhuman_rpc::host::cli(&["http_host".to_string(), "--help".to_string()])
                .expect("the http_host namespace extension is registered");
        })
        .expect("test thread")
        .join()
        .expect("test thread should not panic");
}
