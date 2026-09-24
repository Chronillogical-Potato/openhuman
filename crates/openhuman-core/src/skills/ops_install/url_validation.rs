//! OpenHuman configuration boundary around portable install URL guards.

use super::super::ops_types::WorkflowFrontmatter;

pub const MAX_INSTALL_URL_LEN: usize = tinyskills::MAX_INSTALL_URL_LEN;
const ALLOW_LOCAL_HTTP_ENV: &str = "OPENHUMAN_SKILL_INSTALL_ALLOW_LOCAL_HTTP";

pub(crate) fn normalize_install_url(raw: &str) -> Result<String, String> {
    // ClawHub's file API identifies the requested Markdown file in its query
    // rather than in the URL path. Preserve this OpenHuman-compatible form
    // while keeping the portable validator strict for every other host/path.
    if let Ok(url) = url::Url::parse(raw) {
        let is_clawhub_file_api = url.host_str() == Some("clawhub.ai")
            && url.path().starts_with("/api/v1/skills/")
            && url.path().ends_with("/file");
        if is_clawhub_file_api {
            let markdown_path = url
                .query_pairs()
                .find(|(key, _)| key == "path")
                .map(|(_, value)| value.to_ascii_lowercase().ends_with(".md"))
                .unwrap_or(false);
            if markdown_path {
                return Ok(raw.to_owned());
            }
        }
    }
    tinyskills::normalize_install_url(raw).map_err(|error| error.to_string())
}

pub(crate) fn derive_install_slug(frontmatter: &WorkflowFrontmatter) -> Result<String, String> {
    tinyskills::derive_install_slug(frontmatter).map_err(|error| error.to_string())
}

pub fn validate_install_url(raw: &str) -> Result<(), String> {
    validate_install_url_with_config(raw, read_allow_local_http_env())
}

pub(crate) fn validate_install_url_with_config(
    raw: &str,
    allow_local_http: bool,
) -> Result<(), String> {
    tinyskills::validate_install_url(raw, allow_local_http).map_err(|error| error.to_string())
}

pub(super) fn read_allow_local_http_env() -> bool {
    std::env::var(ALLOW_LOCAL_HTTP_ENV).ok().as_deref() == Some("1")
}

pub(super) fn is_loopback_http_url(raw: &str) -> bool {
    tinyskills::is_loopback_http_url(raw)
}

pub async fn validate_resolved_host(raw_url: &str) -> Result<(), String> {
    tinyskills::validate_resolved_host(raw_url)
        .await
        .map(|_| ())
        .map_err(|error| error.to_string())
}
