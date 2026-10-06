mod error;
mod exchange;
mod pkce;
pub mod providers;
mod server;

pub use error::Error;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore as _;

const LOGIN_TIMEOUT_SECS: u64 = 120;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenBodyFormat {
    Form,
    Json,
}

/// How to derive the OAuth `state` CSRF nonce.
///
/// Most servers treat `state` as opaque and echo it back unchanged. Anthropic's
/// `claude.ai/oauth/authorize` endpoint empirically rejects random `state`
/// values with "Invalid request format"; setting `state` equal to the PKCE
/// verifier (as the Claude Code CLI and `@earendil-works/pi-ai` both do) is
/// accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateStrategy {
    /// Generate a fresh random 16-byte base64url value. Standard OAuth.
    Random,
    /// Reuse the PKCE verifier as the state nonce. Required for Anthropic.
    EqualsVerifier,
}

#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: &'static str,
    pub client_secret: Option<&'static str>,
    pub auth_url: &'static str,
    pub token_url: &'static str,
    pub scopes: &'static [&'static str],
    pub redirect_port: Option<u16>,
    pub callback_path: &'static str,
    pub redirect_uri_host: &'static str,
    pub token_body: TokenBodyFormat,
    pub extra_auth_params: &'static [(&'static str, &'static str)],
    pub state_strategy: StateStrategy,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Token {
    pub access_token: String,
    pub refresh_token: String,
    pub id_token: Option<String>,
    pub expires_in: u64,
    pub issued_at: u64,
}

impl Token {
    pub fn is_expired(&self) -> bool {
        unix_now() + 60 >= self.issued_at + self.expires_in
    }
}

pub async fn login(config: &OAuthConfig) -> Result<Token, Error> {
    let pkce = pkce::Pkce::generate();

    let state = match config.state_strategy {
        StateStrategy::Random => {
            let mut state_bytes = [0u8; 16];
            rand::rng().fill_bytes(&mut state_bytes);
            URL_SAFE_NO_PAD.encode(state_bytes)
        }
        StateStrategy::EqualsVerifier => pkce.verifier.clone(),
    };

    let server = server::bind(config.redirect_port).await?;
    let redirect_uri =
        build_redirect_uri(config.redirect_uri_host, server.port, config.callback_path);

    let auth_url = build_auth_url(config, &pkce.challenge, &state, &redirect_uri);

    println!("Open this URL to log in:\n\n  {auth_url}\n");
    let _ = open_browser(&auth_url);

    let (code, returned_state) = tokio::time::timeout(
        std::time::Duration::from_secs(LOGIN_TIMEOUT_SECS),
        server::wait_for_callback(server, config.callback_path),
    )
    .await
    .map_err(|_| {
        Error::Callback(format!(
            "timed out waiting for browser callback ({LOGIN_TIMEOUT_SECS}s)"
        ))
    })??;

    if returned_state != state {
        return Err(Error::StateMismatch);
    }

    exchange::exchange_code(config, &code, &state, &pkce.verifier, &redirect_uri).await
}

pub async fn refresh(config: &OAuthConfig, refresh_token: &str) -> Result<Token, Error> {
    exchange::refresh_token(config, refresh_token).await
}

pub(crate) fn build_redirect_uri(host: &str, port: u16, path: &str) -> String {
    format!("http://{host}:{port}{path}")
}

pub(crate) fn build_auth_url(
    config: &OAuthConfig,
    challenge: &str,
    state: &str,
    redirect_uri: &str,
) -> String {
    let mut url = reqwest::Url::parse(config.auth_url).expect("auth_url must be valid");
    {
        let mut q = url.query_pairs_mut();
        q.append_pair("client_id", config.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("scope", &config.scopes.join(" "))
            .append_pair("state", state)
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
        for (k, v) in config.extra_auth_params {
            q.append_pair(k, v);
        }
    }
    url.to_string()
}

pub(crate) fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn open_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    std::process::Command::new("open").arg(url).spawn()?;
    #[cfg(target_os = "linux")]
    std::process::Command::new("xdg-open").arg(url).spawn()?;
    #[cfg(target_os = "windows")]
    std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn()?;
    Ok(())
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
