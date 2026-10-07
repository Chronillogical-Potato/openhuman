//! The checks a SaaS core must pass before it boots.
//!
//! Fail closed: [`check`] collects **every** violation and refuses the boot
//! if there is one, so an operator fixes a deployment in one pass instead of
//! one error at a time. It is a pure function of [`BootInputs`] — the
//! environment, home directory and token file are read by the caller — so
//! each rule is testable without touching process state.

use std::path::{Path, PathBuf};

use super::saas::SaasConfig;
use super::{DomainSet, ServiceSet};
use crate::core::types::HostKind;

/// Shortest gateway bearer accepted, in bytes.
pub const MIN_SERVICE_TOKEN_LEN: usize = 32;

/// Environment variables that must not be set for a SaaS core, with why.
///
/// Each one either points the core at a single user's state, hands it a
/// process-wide credential, or opens a debugging back door.
pub const FORBIDDEN_ENV: &[(&str, &str)] = &[
    (
        "OPENHUMAN_WORKSPACE",
        "pins one workspace for the whole process",
    ),
    (
        "OPENHUMAN_DEV_CONNECT",
        "exposes the bearer through /dev/connect",
    ),
    (
        "OPENHUMAN_BACKEND_SESSION_TOKEN",
        "a process-wide user session",
    ),
    (
        "OPENHUMAN_CORE_TOKEN",
        "the gateway bearer must come from the service token file",
    ),
];

/// Switches that are fine on but must not turn a protection off.
pub const FORBIDDEN_OFF_SWITCHES: &[(&str, &str)] = &[
    (
        "OPENHUMAN_APPROVAL_GATE",
        "the approval gate cannot be switched off",
    ),
    ("OPENHUMAN_SANDBOX", "sandboxing cannot be switched off"),
];

fn is_off(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "0" | "false" | "off" | "no" | "disabled"
    )
}

/// A process-wide backend key; allowed only when the operator opts into
/// [`SaasConfig::shared_backend_api_key`].
pub const SHARED_API_KEY_ENV: &str = "OPENHUMAN_BACKEND_API_KEY";

/// What reading the service token file found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceToken {
    /// A bearer that passed every check.
    Valid(String),
    /// Why the file cannot be used.
    Invalid(String),
}

impl ServiceToken {
    /// Read and vet the token at `path`: present, owner-only, and long enough.
    pub fn read(path: &Path) -> Self {
        let display = path.display();
        let raw = match std::fs::read_to_string(path) {
            Ok(raw) => raw,
            Err(e) => return Self::Invalid(format!("cannot read {display}: {e}")),
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            match std::fs::metadata(path) {
                Ok(meta) if meta.permissions().mode() & 0o077 != 0 => {
                    return Self::Invalid(format!(
                        "{display} is readable by others (mode {:o}); chmod 600 it",
                        meta.permissions().mode() & 0o777
                    ));
                }
                Ok(_) => {}
                Err(e) => return Self::Invalid(format!("cannot stat {display}: {e}")),
            }
        }
        let token = raw.trim();
        if token.len() < MIN_SERVICE_TOKEN_LEN {
            return Self::Invalid(format!(
                "{display} holds {} bytes; at least {MIN_SERVICE_TOKEN_LEN} are required",
                token.len()
            ));
        }
        Self::Valid(token.to_string())
    }
}

/// Everything [`check`] decides on.
pub struct BootInputs<'a> {
    pub host_kind: HostKind,
    pub services: ServiceSet,
    pub domains: DomainSet,
    pub config: &'a SaasConfig,
    pub token: &'a ServiceToken,
    pub env: &'a [(String, String)],
    pub home: Option<PathBuf>,
}

/// One reason a SaaS core refuses to boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Violation {
    HostKind(HostKind),
    Service(&'static str),
    Domain(&'static str),
    Env { var: String, why: &'static str },
    ToolAllowlist(Vec<String>),
    RpcAllowlist(Vec<String>),
    Root(String),
    ServiceToken(String),
}

impl std::fmt::Display for Violation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HostKind(kind) => write!(f, "host kind `{}` cannot serve SaaS", kind.tag()),
            Self::Service(name) => write!(f, "service `{name}` is not allowed in SaaS mode"),
            Self::Domain(name) => write!(f, "domain family `{name}` is not isolated per user yet"),
            Self::Env { var, why } => write!(f, "environment variable {var} is set: {why}"),
            Self::ToolAllowlist(groups) => write!(
                f,
                "tool_allowlist {groups:?}: no tool group can be opted in until per-user sandboxing ships"
            ),
            Self::RpcAllowlist(methods) => write!(
                f,
                "rpc_allowlist_extra {methods:?}: no RPC method can be opted in until the per-user RPC surface ships"
            ),
            Self::Root(why) => write!(f, "root: {why}"),
            Self::ServiceToken(why) => write!(f, "service token: {why}"),
        }
    }
}

/// The boot was refused; every violation found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootGuardError {
    pub violations: Vec<Violation>,
}

impl std::fmt::Display for BootGuardError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(
            f,
            "[saas] refusing to boot: {} problem(s) with this deployment",
            self.violations.len()
        )?;
        for violation in &self.violations {
            writeln!(f, "  - {violation}")?;
        }
        Ok(())
    }
}

impl std::error::Error for BootGuardError {}

/// Refuse a SaaS boot that is not safe to serve users from.
pub fn check(inputs: &BootInputs<'_>) -> Result<(), BootGuardError> {
    let mut violations = Vec::new();

    if !matches!(inputs.host_kind, HostKind::Saas) {
        violations.push(Violation::HostKind(inputs.host_kind));
    }

    let allowed = ServiceSet::saas();
    for ((name, on), (_, ok)) in service_flags(&inputs.services)
        .into_iter()
        .zip(service_flags(&allowed))
    {
        if on && !ok {
            violations.push(Violation::Service(name));
        }
    }
    let allowed = DomainSet::saas();
    for ((name, on), (_, ok)) in domain_flags(&inputs.domains)
        .into_iter()
        .zip(domain_flags(&allowed))
    {
        if on && !ok {
            violations.push(Violation::Domain(name));
        }
    }

    for (var, value) in inputs.env {
        if value.trim().is_empty() {
            continue;
        }
        if let Some((_, why)) = FORBIDDEN_ENV.iter().find(|(name, _)| name == var) {
            violations.push(Violation::Env {
                var: var.clone(),
                why,
            });
        } else if let Some((_, why)) = FORBIDDEN_OFF_SWITCHES
            .iter()
            .find(|(name, _)| name == var && is_off(value))
        {
            violations.push(Violation::Env {
                var: var.clone(),
                why,
            });
        } else if var == SHARED_API_KEY_ENV && !inputs.config.shared_backend_api_key {
            violations.push(Violation::Env {
                var: var.clone(),
                why: "a process-wide backend key; set shared_backend_api_key to allow it",
            });
        }
    }

    if !inputs.config.tool_allowlist.is_empty() {
        violations.push(Violation::ToolAllowlist(
            inputs.config.tool_allowlist.clone(),
        ));
    }
    if !inputs.config.rpc_allowlist_extra.is_empty() {
        violations.push(Violation::RpcAllowlist(
            inputs.config.rpc_allowlist_extra.clone(),
        ));
    }

    if let Some(why) = root_problem(&inputs.config.root, inputs.home.as_deref()) {
        violations.push(Violation::Root(why));
    }
    if let ServiceToken::Invalid(why) = inputs.token {
        violations.push(Violation::ServiceToken(why.clone()));
    }

    if violations.is_empty() {
        log::info!("[saas][boot-guard] deployment checks passed");
        Ok(())
    } else {
        for violation in &violations {
            log::error!("[saas][boot-guard] {violation}");
        }
        Err(BootGuardError { violations })
    }
}

fn root_problem(root: &Path, home: Option<&Path>) -> Option<String> {
    if !root.is_absolute() {
        return Some(format!("{} is not an absolute path", root.display()));
    }
    let meta = match std::fs::metadata(root) {
        Ok(meta) if meta.is_dir() => meta,
        Ok(_) => return Some(format!("{} is not a directory", root.display())),
        Err(e) => return Some(format!("{}: {e}", root.display())),
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o002 != 0 {
            return Some(format!("{} is world-writable", root.display()));
        }
    }
    #[cfg(not(unix))]
    let _ = meta;
    if let Some(home) = home {
        let desktop_root = home.join(".openhuman");
        if root.starts_with(&desktop_root) {
            return Some(format!(
                "{} is inside the desktop app's {}",
                root.display(),
                desktop_root.display()
            ));
        }
    }
    None
}

fn service_flags(s: &ServiceSet) -> [(&'static str, bool); 12] {
    [
        ("rpc_http", s.rpc_http),
        ("socketio", s.socketio),
        ("cron", s.cron),
        ("channels", s.channels),
        ("login_gated", s.login_gated),
        ("update_scheduler", s.update_scheduler),
        ("memory_queue", s.memory_queue),
        ("harness_init", s.harness_init),
        ("skill_catalog_refresh", s.skill_catalog_refresh),
        ("mcp_boot", s.mcp_boot),
        ("integrations", s.integrations),
        ("memory_sync", s.memory_sync),
    ]
}

fn domain_flags(d: &DomainSet) -> [(&'static str, bool); 20] {
    [
        ("agent", d.agent),
        ("memory", d.memory),
        ("threads", d.threads),
        ("config", d.config),
        ("security", d.security),
        ("flows", d.flows),
        ("skills", d.skills),
        ("mcp", d.mcp),
        ("channels", d.channels),
        ("web3", d.web3),
        ("voice", d.voice),
        ("media", d.media),
        ("inference", d.inference),
        ("integrations", d.integrations),
        ("automation", d.automation),
        ("runtimes", d.runtimes),
        ("desktop", d.desktop),
        ("hosted", d.hosted),
        ("modules", d.modules),
        ("platform", d.platform),
    ]
}

#[cfg(test)]
#[path = "boot_guard_tests.rs"]
mod tests;
