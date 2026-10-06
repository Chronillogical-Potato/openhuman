use super::*;

#[test]
fn codex_config_has_correct_client_id() {
    let c = codex();
    assert_eq!(c.client_id, "app_EMoamEEZ73f0CkXaXp7hrann");
}

#[test]
fn codex_config_has_no_client_secret() {
    let c = codex();
    assert!(c.client_secret.is_none());
}

#[test]
fn codex_config_redirect_port_is_1455() {
    let c = codex();
    assert_eq!(c.redirect_port, Some(1455));
}

#[test]
fn codex_config_auth_url_is_openai() {
    let c = codex();
    assert!(c.auth_url.contains("auth.openai.com"));
}
