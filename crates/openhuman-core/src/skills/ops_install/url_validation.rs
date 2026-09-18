//! OpenHuman configuration boundary around portable install URL guards.

use super::super::ops_types::WorkflowFrontmatter;

pub const MAX_INSTALL_URL_LEN: usize = tinyskills::install::MAX_INSTALL_URL_LEN;
const ALLOW_LOCAL_HTTP_ENV: &str = "OPENHUMAN_SKILL_INSTALL_ALLOW_LOCAL_HTTP";

pub(crate) fn normalize_install_url(raw: &str) -> Result<String, String> {
    tinyskills::normalize_install_url(raw)
}

pub(crate) fn derive_install_slug(frontmatter: &WorkflowFrontmatter) -> Result<String, String> {
    tinyskills::derive_install_slug(frontmatter)
}

pub fn validate_install_url(raw: &str) -> Result<(), String> {
    validate_install_url_with_config(raw, read_allow_local_http_env())
}

pub(crate) fn validate_install_url_with_config(
    raw: &str,
    allow_local_http: bool,
) -> Result<(), String> {
    tinyskills::validate_install_url(raw, allow_local_http)
}

pub(super) fn read_allow_local_http_env() -> bool {
    std::env::var(ALLOW_LOCAL_HTTP_ENV).ok().as_deref() == Some("1")
}

pub(super) fn is_loopback_http_url(raw: &str) -> bool {
    tinyskills::install::is_loopback_http_url(raw)
}

pub async fn validate_resolved_host(raw_url: &str) -> Result<(), String> {
    tinyskills::validate_resolved_host(raw_url).await
}
