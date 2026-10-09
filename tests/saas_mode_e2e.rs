//! `openhuman-core run --mode saas` end to end, through the real binary.
//!
//! A SaaS core must refuse an unsafe deployment before it binds anything, and
//! a safe one must serve nothing but its core built-ins behind the gateway
//! bearer until per-user isolation opens domain families.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const BEARER: &str = "saas-e2e-gateway-bearer-0123456789abcdef";

/// Environment a developer machine may carry that would point the child at a
/// single user, or that the boot guard refuses outright.
const SCRUBBED_ENV: &[&str] = &[
    "OPENHUMAN_WORKSPACE",
    "OPENHUMAN_DEV_CONNECT",
    "OPENHUMAN_BACKEND_SESSION_TOKEN",
    "OPENHUMAN_BACKEND_API_KEY",
    "OPENHUMAN_CORE_TOKEN",
    "OPENHUMAN_APPROVAL_GATE",
    "OPENHUMAN_SANDBOX",
    "OPENHUMAN_MODE",
];

struct Deployment {
    tmp: tempfile::TempDir,
    root: PathBuf,
    config: PathBuf,
}

fn deployment(write_token: bool) -> Deployment {
    let tmp = tempfile::tempdir().expect("temp dir");
    let root = tmp.path().join("saas");
    std::fs::create_dir(&root).unwrap();
    set_mode(&root, 0o700);
    if write_token {
        let token = root.join("service.token");
        std::fs::write(&token, format!("{BEARER}\n")).unwrap();
        set_mode(&token, 0o600);
    }
    let config = tmp.path().join("operator.toml");
    std::fs::write(
        &config,
        format!("root = {:?}\n", root.display().to_string()),
    )
    .unwrap();
    Deployment { tmp, root, config }
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(not(unix))]
fn set_mode(_: &Path, _: u32) {}

fn core_command(d: &Deployment, extra: &[&str]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_openhuman-core"));
    cmd.args(["run", "--mode", "saas", "--saas-config"])
        .arg(&d.config)
        .args(extra)
        // Keep the child away from the developer's real `~/.openhuman`.
        .env("HOME", d.tmp.path())
        .env("USERPROFILE", d.tmp.path());
    for var in SCRUBBED_ENV {
        cmd.env_remove(var);
    }
    cmd
}

#[test]
fn an_unsafe_deployment_is_refused_before_it_binds() {
    let d = deployment(false);
    let output = core_command(&d, &[])
        .env("OPENHUMAN_WORKSPACE", d.tmp.path())
        .output()
        .expect("run openhuman-core");
    assert!(!output.status.success(), "an unsafe SaaS boot must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refusing to boot"), "{stderr}");
    assert!(stderr.contains("service token"), "{stderr}");
    assert!(stderr.contains("OPENHUMAN_WORKSPACE"), "{stderr}");
    assert!(
        !d.root.join("operator").exists(),
        "a refused boot must not create state"
    );
}

#[test]
fn saas_mode_without_an_operator_config_is_refused() {
    let output = Command::new(env!("CARGO_BIN_EXE_openhuman-core"))
        .args(["run", "--mode", "saas"])
        .env_remove("OPENHUMAN_MODE")
        .output()
        .expect("run openhuman-core");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--saas-config"), "{stderr}");
}

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn rpc(
    client: &reqwest::blocking::Client,
    base: &str,
    bearer: Option<&str>,
    method: &str,
) -> (u16, Value) {
    let mut request = client.post(format!("{base}/rpc")).json(&json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": method,
        "params": {}
    }));
    if let Some(bearer) = bearer {
        request = request.bearer_auth(bearer);
    }
    let response = request.send().expect("POST /rpc");
    let status = response.status().as_u16();
    (status, response.json().unwrap_or(Value::Null))
}

#[test]
fn a_safe_deployment_serves_only_core_built_ins_behind_the_gateway_bearer() {
    let d = deployment(true);
    let port = free_port();
    let child = core_command(&d, &["--port", &port.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn openhuman-core");
    let mut server = Server(child);
    let base = format!("http://127.0.0.1:{port}");
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();

    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        if let Ok(response) = client.get(format!("{base}/health")).send() {
            if response.status().is_success() {
                break;
            }
        }
        if let Ok(Some(status)) = server.0.try_wait() {
            panic!("SaaS core exited before serving: {status}");
        }
        assert!(Instant::now() < deadline, "SaaS core never became healthy");
        std::thread::sleep(Duration::from_millis(250));
    }

    let (status, _) = rpc(&client, &base, None, "core.ping");
    assert_eq!(status, 401, "no bearer, no access");
    let (status, _) = rpc(&client, &base, Some("wrong-bearer"), "core.ping");
    assert_eq!(status, 401, "only the gateway bearer is accepted");

    let (status, body) = rpc(&client, &base, Some(BEARER), "core.ping");
    assert_eq!(status, 200);
    assert!(body.get("result").is_some(), "core.ping answers: {body}");

    // No domain family is isolated per user yet, so none is served.
    for method in [
        "openhuman.threads_list",
        "openhuman.config_get_config",
        "openhuman.memory_search",
    ] {
        let (_, body) = rpc(&client, &base, Some(BEARER), method);
        assert!(
            body.get("error").is_some(),
            "{method} must not be served: {body}"
        );
    }

    assert!(
        d.root.join("operator").join("workspace").is_dir(),
        "the operator plane lives under the SaaS root"
    );
    assert!(
        !d.tmp
            .path()
            .join(".openhuman")
            .join("active_user.toml")
            .exists(),
        "a SaaS boot never activates a desktop user"
    );
    let desktop = d.tmp.path().join(".openhuman");
    let leaked: Vec<_> = std::fs::read_dir(&desktop)
        .map(|entries| entries.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(
        leaked.is_empty(),
        "a SaaS boot writes nothing under ~/.openhuman (keyring included): {leaked:?}"
    );
    drop(server);
}
