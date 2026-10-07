//! Host adapter for the browser members of the loadable TinyComputer module.
//!
//! TinyComputer serves its browser members (`BrowserOpenSession`,
//! `BrowserNavigate`, ...) on the same interface as its desktop members, so
//! this adapter shares the desktop adapter's proxy, which loads the module and
//! refreshes its private configuration before a call.

use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use tinycomputer_bus::agent::Capabilities;
use tinycomputer_bus::browser::{
    names, Action, ActionOutcome, DownloadInfo, DownloadWaitRequest, NavigateRequest, PageState,
    PageText, ReadRequest, SessionId, SessionInfo, SessionOptions, SessionRef, SessionRequest,
    Snapshot, SnapshotRequest, Viewport,
};
use tinycomputer_bus::DesktopResponse;

use crate::config::Config;

pub const MODULE_ID: &str = super::desktop::MODULE_ID;

#[derive(Debug, thiserror::Error)]
pub enum BrowserCallError {
    #[error("TinyComputer browser unavailable: {0}")]
    Unavailable(String),
    #[error("browser request blocked: {0}")]
    Policy(String),
    #[error("{name}: {message}")]
    Bus { name: String, message: String },
}

impl BrowserCallError {
    fn from_bus(error: tinybus::Error) -> Self {
        Self::Bus {
            name: error.wire_name().to_owned(),
            message: error.to_string(),
        }
    }

    /// A failed reply names its error in `details.name` (the
    /// `ai.tinyhumans.tinycomputer.Browser.Error.*` spelling); older replies
    /// carry only the envelope code.
    fn from_response(response: &DesktopResponse) -> Self {
        let Some(error) = response.error.as_ref() else {
            return Self::Bus {
                name: "ModuleFailed".into(),
                message: format!("{} failed without an error", response.command),
            };
        };
        let name = error
            .details
            .as_ref()
            .and_then(|details| details.get("name"))
            .and_then(serde_json::Value::as_str)
            .map(|name| name.rsplit('.').next().unwrap_or(name).to_owned())
            .unwrap_or_else(|| error.code.clone());
        Self::Bus {
            name,
            message: error.message.clone(),
        }
    }
}

/// A TinyComputer browser connection that applies this host's settings and shared web policy.
#[derive(Clone)]
pub struct BrowserClient {
    config: Arc<Config>,
}

impl BrowserClient {
    pub fn new(config: Arc<Config>) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub async fn ensure_ready(&self) -> Result<(), BrowserCallError> {
        let version = self.contract_version().await?;
        if !tinycomputer_bus::is_compatible(version) {
            return Err(BrowserCallError::Unavailable(format!(
                "incompatible TinyComputer contract: module {version:?}, host {:?}",
                tinycomputer_bus::CONTRACT_VERSION
            )));
        }
        Ok(())
    }

    /// The module's contract version, from `Describe` (which answers raw
    /// [`Capabilities`], not an envelope).
    pub async fn contract_version(&self) -> Result<(u32, u32), BrowserCallError> {
        let proxy = super::desktop::proxy(&self.config)
            .await
            .map_err(BrowserCallError::Unavailable)?;
        let capabilities: Capabilities = proxy
            .call(tinycomputer_bus::agent::names::methods::DESCRIBE, ())
            .await
            .map_err(BrowserCallError::from_bus)?;
        Ok(capabilities.contract_version)
    }

    pub async fn open_session(
        &self,
        options: SessionOptions,
    ) -> Result<SessionInfo, BrowserCallError> {
        self.open_session_with_origin(options, None).await
    }

    async fn open_session_with_origin(
        &self,
        mut options: SessionOptions,
        origin: Option<String>,
    ) -> Result<SessionInfo, BrowserCallError> {
        self.ensure_ready().await?;
        let domains = self.browser_domains();
        if domains.is_empty() && !browser_allow_all() {
            return Err(BrowserCallError::Policy(
                "allowed websites list is empty".into(),
            ));
        }
        let browser = &self.config.browser;
        if browser.profile_mode == "persistent"
            && !browser
                .profile_path
                .as_ref()
                .is_some_and(|path| std::path::Path::new(path).is_absolute())
        {
            return Err(BrowserCallError::Policy(
                "persistent profile requires an absolute profile_path".into(),
            ));
        }
        if browser
            .download_dir
            .as_ref()
            .is_some_and(|path| !std::path::Path::new(path).is_absolute())
        {
            return Err(BrowserCallError::Policy(
                "download_dir must be absolute".into(),
            ));
        }
        options.headless = browser.headless;
        options.viewport = Viewport::desktop(browser.viewport_width, browser.viewport_height);
        options.executable = browser.chrome_path.clone();
        options.user_data_dir = if browser.profile_mode == "persistent" {
            browser.profile_path.clone()
        } else {
            None
        };
        options.download_dir = browser.download_dir.clone();
        options.default_timeout_ms = browser.task_timeout_secs.saturating_mul(1000);
        options.allowed_origins =
            origin.map_or_else(|| self.browser_origins(), |origin| vec![origin]);
        self.call(names::methods::OPEN_SESSION, options).await
    }

    /// Open a session bound to the host tree of an explicit destination.
    pub async fn open_session_for_url(&self, url: &str) -> Result<SessionInfo, BrowserCallError> {
        let origin = self.explicit_origin(url)?;
        self.open_session_with_origin(SessionOptions::default(), origin)
            .await
    }

    pub(crate) fn explicit_origin(&self, raw: &str) -> Result<Option<String>, BrowserCallError> {
        self.explicit_origin_for_mode(raw, browser_allow_all())
    }

    fn explicit_origin_for_mode(
        &self,
        raw: &str,
        allow_all: bool,
    ) -> Result<Option<String>, BrowserCallError> {
        self.check_url_for_mode(raw, allow_all)?;
        if !allow_all {
            return Ok(None);
        }
        let url = parsed_browser_url(raw)?;
        let host = url.host().expect("validated URL has a host");
        Ok(Some(match host {
            url::Host::Domain(host) => format!("https://.{}", host.trim_end_matches('.')),
            url::Host::Ipv4(_) | url::Host::Ipv6(_) => url.origin().ascii_serialization(),
        }))
    }

    pub async fn navigate(
        &self,
        session: &SessionId,
        request: NavigateRequest,
    ) -> Result<PageState, BrowserCallError> {
        self.check_url(&request.url)?;
        let page: PageState = self
            .call(
                names::methods::NAVIGATE,
                SessionRequest::new(session.clone(), request),
            )
            .await?;
        self.check_returned_url(&page.url)?;
        Ok(page)
    }

    pub async fn snapshot(
        &self,
        session: &SessionId,
        request: SnapshotRequest,
    ) -> Result<Snapshot, BrowserCallError> {
        let snapshot: Snapshot = self
            .call(
                names::methods::SNAPSHOT,
                SessionRequest::new(session.clone(), request),
            )
            .await?;
        self.check_returned_url(&snapshot.url)?;
        Ok(snapshot)
    }

    pub async fn perform(
        &self,
        session: &SessionId,
        action: Action,
    ) -> Result<ActionOutcome, BrowserCallError> {
        let outcome: ActionOutcome = self
            .call(
                names::methods::PERFORM,
                SessionRequest::new(session.clone(), action),
            )
            .await?;
        self.check_returned_url(&outcome.page.url)?;
        Ok(outcome)
    }

    pub async fn read_page(
        &self,
        session: &SessionId,
        request: ReadRequest,
    ) -> Result<PageText, BrowserCallError> {
        let page: PageText = self
            .call(
                names::methods::READ_PAGE,
                SessionRequest::new(session.clone(), request),
            )
            .await?;
        self.check_returned_url(&page.url)?;
        Ok(page)
    }

    pub async fn list_downloads(
        &self,
        session: &SessionId,
    ) -> Result<Vec<DownloadInfo>, BrowserCallError> {
        self.call(names::methods::LIST_DOWNLOADS, session_ref(session))
            .await
    }

    pub async fn wait_download(
        &self,
        session: &SessionId,
        request: DownloadWaitRequest,
    ) -> Result<DownloadInfo, BrowserCallError> {
        self.call(
            names::methods::WAIT_DOWNLOAD,
            SessionRequest::new(session.clone(), request),
        )
        .await
    }

    pub async fn close_session(&self, session: &SessionId) -> Result<(), BrowserCallError> {
        let _closed: serde_json::Value = self
            .call(names::methods::CLOSE_SESSION, session_ref(session))
            .await?;
        Ok(())
    }

    pub(crate) fn check_url(&self, raw: &str) -> Result<(), BrowserCallError> {
        self.check_url_for_mode(raw, browser_allow_all())
    }

    fn check_url_for_mode(&self, raw: &str, allow_all: bool) -> Result<(), BrowserCallError> {
        if raw.trim().is_empty() || raw.chars().any(char::is_whitespace) {
            return Err(BrowserCallError::Policy("invalid URL".into()));
        }
        let url = parsed_browser_url(raw)?;
        if url.scheme() != "https" {
            return Err(BrowserCallError::Policy(
                "only HTTPS URLs are allowed".into(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(BrowserCallError::Policy(
                "URL userinfo is not allowed".into(),
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| BrowserCallError::Policy("URL has no host".into()))?;
        let is_private = match url.host() {
            Some(url::Host::Ipv4(ip)) => private_or_local(&ip.to_string()),
            Some(url::Host::Ipv6(ip)) => private_or_local(&ip.to_string()),
            _ => private_or_local(host),
        };
        if is_private {
            return Err(BrowserCallError::Policy(format!(
                "local/private host '{host}' is blocked"
            )));
        }
        if !allow_all
            && !self.browser_domains().iter().any(|allowed| {
                host.eq_ignore_ascii_case(allowed)
                    || host
                        .to_ascii_lowercase()
                        .ends_with(&format!(".{}", allowed.to_ascii_lowercase()))
            })
        {
            return Err(BrowserCallError::Policy(format!(
                "host '{host}' is not in allowed websites"
            )));
        }
        Ok(())
    }

    fn check_returned_url(&self, raw: &str) -> Result<(), BrowserCallError> {
        // A fresh session starts at about:blank. It is internal to Chrome and
        // does not make a network request; reporting it lets the agent see it
        // needs to navigate before inspecting page content.
        if raw.eq_ignore_ascii_case("about:blank") {
            return Ok(());
        }
        self.check_url(raw)
    }

    fn browser_domains(&self) -> Vec<String> {
        self.config
            .http_request
            .allowed_domains
            .iter()
            .filter(|domain| domain.as_str() != "*")
            .cloned()
            .collect()
    }

    /// Origins a TinyComputer task may load: the allowed websites, in the
    /// module's `https://.host` spelling. Empty when none are allowed, which a
    /// task refuses, since an empty list means any origin to the module.
    pub(crate) fn task_origins(&self) -> Vec<String> {
        self.browser_origins()
    }

    fn browser_origins(&self) -> Vec<String> {
        self.browser_origins_for_mode(browser_allow_all())
    }

    fn browser_origins_for_mode(&self, allow_all: bool) -> Vec<String> {
        if allow_all {
            // A nonempty, unmatchable origin keeps TinyComputer's document
            // guard active before the caller supplies an explicit URL.
            return vec!["https://.".to_owned()];
        }
        self.browser_domains()
            .iter()
            .map(|host| {
                if host.contains("://") {
                    host.clone()
                } else {
                    format!("https://.{}", host.trim_start_matches('.'))
                }
            })
            .collect()
    }

    async fn call<A: Serialize + Send, R: DeserializeOwned>(
        &self,
        member: &str,
        request: A,
    ) -> Result<R, BrowserCallError> {
        let proxy = super::desktop::proxy(&self.config)
            .await
            .map_err(BrowserCallError::Unavailable)?;
        tracing::debug!(member, "[browser] tinycomputer call");
        let response: DesktopResponse = proxy
            .call(member, (request,))
            .await
            .map_err(BrowserCallError::from_bus)?;
        if !response.ok {
            let error = BrowserCallError::from_response(&response);
            tracing::debug!(member, error = %error, "[browser] tinycomputer call failed");
            return Err(error);
        }
        serde_json::from_value(response.data.unwrap_or(serde_json::Value::Null)).map_err(|error| {
            BrowserCallError::Bus {
                name: "InvalidReply".into(),
                message: format!("{member} returned an unexpected reply: {error}"),
            }
        })
    }
}

fn session_ref(session: &SessionId) -> SessionRef {
    SessionRef {
        session: session.clone(),
    }
}

fn parsed_browser_url(raw: &str) -> Result<url::Url, BrowserCallError> {
    url::Url::parse(raw)
        .or_else(|_| url::Url::parse(&format!("https://{raw}")))
        .map_err(|_| BrowserCallError::Policy("invalid URL".into()))
}

fn browser_allow_all() -> bool {
    matches!(
        std::env::var("OPENHUMAN_BROWSER_ALLOW_ALL").ok().as_deref(),
        Some("1" | "true" | "TRUE" | "yes" | "YES")
    )
}

fn private_or_local(host: &str) -> bool {
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host == "localhost" || host.ends_with(".localhost") || host.ends_with(".local") {
        return true;
    }
    // The network tools' full non-global list, so the browser blocks the same
    // reserved, benchmarking and documentation ranges they do.
    match host.parse::<std::net::IpAddr>() {
        // `is_non_global_v4` already covers 0.0.0.0/8; the explicit octet check
        // keeps that local-host range visibly blocked at this call site too.
        Ok(std::net::IpAddr::V4(ip)) => {
            tinytools_std::url_guard::is_non_global_v4(ip) || ip.octets()[0] == 0
        }
        Ok(std::net::IpAddr::V6(ip)) => tinytools_std::url_guard::is_non_global_v6(ip),
        Err(_) => false,
    }
}

#[cfg(test)]
#[path = "browser_tests.rs"]
mod tests;
