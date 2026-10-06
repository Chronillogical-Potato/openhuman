use crate::{OAuthConfig, StateStrategy, TokenBodyFormat};

pub fn codex() -> OAuthConfig {
    OAuthConfig {
        client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
        client_secret: None,
        auth_url: "https://auth.openai.com/oauth/authorize",
        token_url: "https://auth.openai.com/oauth/token",
        scopes: &["openid", "profile", "email", "offline_access"],
        redirect_port: Some(1455),
        callback_path: "/auth/callback",
        redirect_uri_host: "127.0.0.1",
        token_body: TokenBodyFormat::Form,
        extra_auth_params: &[("access_type", "offline")],
        state_strategy: StateStrategy::Random,
    }
}

#[cfg(test)]
#[path = "codex_tests.rs"]
mod tests;
