//! `web_fetch` — fetch a URL and return its text body.
//!
//! Coding-harness baseline tool (issue #1205). Distinct from
//! `http_request` (full method/header surface) and `curl` (writes to
//! disk). `web_fetch` is the single-purpose "GET and read" primitive
//! the agent reaches for when researching: returns the response body
//! as text, capped, with a tiny preamble (status + final URL).

use super::url_guard::{normalize_allowed_domains, validate_url_with_dns_check};
use crate::config::HttpRequestConfig;
use crate::security::SecurityPolicy;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tinytools::{PermissionLevel, Tool, ToolResult};

pub struct WebFetchTool {
    security: Arc<SecurityPolicy>,
    allowed_domains: Vec<String>,
    max_bytes: usize,
    timeout_secs: u64,
}

impl WebFetchTool {
    pub fn new(
        security: Arc<SecurityPolicy>,
        allowed_domains: Vec<String>,
        max_bytes: Option<usize>,
        timeout_secs: Option<u64>,
    ) -> Self {
        // Treat both `None` and `Some(0)` as "use default": callers wire these
        // from `[http_request]`, and a 0-byte cap truncates every body to
        // nothing while a 0-second timeout fails every request instantly.
        // Stale-zero configs are repaired on load (migration 5→6); this clamp
        // is the always-on guard at the point of use. Pull the fallbacks from
        // `HttpRequestConfig::default()` so the tool shares one source with the
        // schema + migration (no cross-layer drift). `Some(0)` is a genuine
        // misconfiguration, so log it (grep-friendly, no payload); a bare
        // `None` is a normal "use default" call and stays quiet.
        let defaults = HttpRequestConfig::default();
        let max_bytes = match max_bytes {
            Some(0) => {
                log::warn!(
                    "[tool.web_fetch] coercing invalid limit field=max_bytes \
                     from=0 to={} (stale/invalid config — see migration 5→6)",
                    defaults.max_response_size
                );
                defaults.max_response_size
            }
            Some(n) => n,
            None => defaults.max_response_size,
        };
        let timeout_secs = match timeout_secs {
            Some(0) => {
                log::warn!(
                    "[tool.web_fetch] coercing invalid limit field=timeout_secs \
                     from=0 to={} (stale/invalid config — see migration 5→6)",
                    defaults.timeout_secs
                );
                defaults.timeout_secs
            }
            Some(n) => n,
            None => defaults.timeout_secs,
        };
        Self {
            security,
            allowed_domains: normalize_allowed_domains(allowed_domains),
            max_bytes,
            timeout_secs,
        }
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn name(&self) -> &str {
        "web_fetch"
    }

    fn description(&self) -> &str {
        "GET a URL and read the page. HTML comes back as Markdown — headings, \
         links and code blocks kept, scripts and styling dropped — so the \
         links in the result are the ones to fetch next. Use this for docs, \
         READMEs and spec pages. Pass `raw: true` for the untouched body \
         (needed for JSON APIs you want verbatim). For richer HTTP semantics \
         (POST, custom headers, …) use `http_request`."
    }

    fn parameters_schema(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Absolute http(s) URL." },
                "max_bytes": {
                    "type": "integer",
                    "description": "Cap the downloaded body at this many bytes (default 1_000_000).",
                    "minimum": 1
                },
                "raw": {
                    "type": "boolean",
                    "description": "Skip HTML-to-Markdown conversion and return the body as sent."
                }
            },
            "required": ["url"]
        })
    }

    fn permission_level(&self) -> PermissionLevel {
        PermissionLevel::ReadOnly
    }

    /// Idempotent GET — safe to fan out across parallel `web_fetch`
    /// calls. Targets that throttle aggressively are the user's
    /// concern; we don't try to second-guess at the tool layer.
    fn is_concurrency_safe(&self, _args: &serde_json::Value) -> bool {
        true
    }

    /// Cap web_fetch results at ~50k chars before they reach the
    /// model. The tool itself already truncates byte-wise via
    /// `max_bytes` (default 1MB), but a 1MB HTML page is still tens
    /// of thousands of tokens — the agent rarely needs that much, and
    /// when it does, `read_file` on a saved copy is the right tool.
    fn max_result_size_chars(&self) -> Option<usize> {
        Some(50_000)
    }

    async fn execute(&self, args: serde_json::Value) -> anyhow::Result<ToolResult> {
        let raw_url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'url' parameter"))?;
        let max_bytes = args
            .get("max_bytes")
            .and_then(|v| v.as_u64())
            .map(|n| (n as usize).max(1))
            .unwrap_or(self.max_bytes);
        let raw_requested = args.get("raw").and_then(|v| v.as_bool()).unwrap_or(false);

        if self.security.is_rate_limited() {
            return Ok(ToolResult::error(
                "Rate limit exceeded: too many actions in the last hour",
            ));
        }
        if !self.security.record_action() {
            return Ok(ToolResult::error(
                "Rate limit exceeded: action budget exhausted",
            ));
        }

        // Local-only enforcement (privacy epic S7, #4441): refuse the fetch under
        // LocalOnly before URL validation / DNS. The post-validation
        // `emit_external_transfer` below stays the S2 disclosure point.
        {
            let host = reqwest::Url::parse(raw_url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .unwrap_or_else(|| "unknown".to_string());
            if let Some(msg) = crate::security::egress::local_only_tool_block(
                &crate::security::egress::EgressDescriptor::network_fetch(host),
            ) {
                return Ok(ToolResult::error(msg));
            }
        }

        let url = match validate_url_with_dns_check(raw_url, &self.allowed_domains).await {
            Ok(u) => u,
            Err(e) => return Ok(ToolResult::error(format!("URL rejected: {e}"))),
        };

        // Egress spine (privacy epic S2, #4436): disclose the fetch destination
        // before contacting the host.
        {
            let host = reqwest::Url::parse(&url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_string))
                .unwrap_or_else(|| "unknown".to_string());
            crate::security::egress::emit_external_transfer(
                crate::security::egress::EgressDescriptor::network_fetch(host),
            );
        }

        // Disable automatic redirect following: reqwest follows up to 10
        // redirects by default, and a redirect target may be on a host
        // outside the allowed-domains list. We surface 3xx responses to
        // the caller so they can decide whether to refetch the new URL.
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(self.timeout_secs))
            .redirect(reqwest::redirect::Policy::none())
            .build()
        {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(format!("Failed to build client: {e}"))),
        };

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => return Ok(ToolResult::error(format!("Request failed: {e}"))),
        };
        let status = resp.status();
        let final_url = resp.url().to_string();
        let location = resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let content_type = resp
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = match resp.text().await {
            Ok(b) => b,
            Err(e) => return Ok(ToolResult::error(format!("Failed to read body: {e}"))),
        };

        if let Some(loc) = &location {
            if status.is_redirection() {
                return Ok(ToolResult::success(format!(
                    "status={} url={} location={loc}\n[redirect not followed — re-call web_fetch with the location URL if it's an allowed domain]",
                    status.as_u16(),
                    final_url
                )));
            }
        }

        let downloaded = body.len();
        let (body, byte_capped) = if downloaded > max_bytes {
            let cut = crate::util::floor_char_boundary(&body, max_bytes);
            (body[..cut].to_string(), true)
        } else {
            (body, false)
        };

        // Markdown by default. A page's prose is a small fraction of its
        // bytes; handing the raw document to the model (and to the payload
        // summarizer behind it) is how one research turn came to cost
        // 1,083,069 input tokens. `tinyjuice` owns content transforms — see
        // the dependency note in Cargo.toml for why this is a direct call and
        // not a trip through the module bus.
        let converted = !raw_requested && is_html(&body, content_type.as_deref());
        let content = if converted {
            tinyjuice::compressors::html::html_to_markdown(&body)
        } else {
            body
        };

        let extracted = content.len();
        let (window, elided) = head_tail_window(&content, MAX_CONTENT_CHARS);

        let mut header = format!("status={} url={final_url}", status.as_u16());
        if converted {
            header.push_str(" content=markdown");
        }
        if byte_capped {
            header.push_str(&format!(" download_capped_at={max_bytes}B"));
        }
        if converted && extracted < downloaded {
            header.push_str(&format!(" extracted={extracted}B_of_{downloaded}B"));
        }
        header.push('\n');

        // Say how much is missing rather than letting the gap pass for the
        // whole page. `web_fetch` has no offset parameter, so the honest
        // instruction is to narrow the request.
        let suffix = match elided {
            0 => String::new(),
            n => format!(
                "\n\n[web_fetch: {n} of {extracted} chars omitted from the middle of this page. \
                 The head and tail are shown. Fetch a more specific URL — an anchor, a sub-page, \
                 or a raw/ or /api/ path — to read the part you need.]"
            ),
        };
        Ok(ToolResult::success(format!("{header}{window}{suffix}")))
    }
}

/// How much extracted content reaches the model.
///
/// Markdown, not markup: after extraction a long documentation page is
/// typically a few thousand chars, so this bites only on genuinely large
/// documents. Sized against the 4,000-token (~16 KB) payload-summarizer
/// trigger in `context.summarizer_payload_threshold_tokens` — a page that
/// survives this window is one the summarizer would otherwise be handed
/// whole. Hermes' `web_extract` uses 15,000 chars of clean markdown for the
/// same job; Codex caps every tool result at ~10,000 tokens.
const MAX_CONTENT_CHARS: usize = 24_000;

/// Fraction of the window spent on the head. A page's lede, title and
/// navigation-to-content transition are front-loaded; the tail is where
/// references, footnotes and "next page" links live. Hermes splits 75/25,
/// Codex 50/50 — exec output puts its verdict last, prose does not.
const HEAD_FRACTION: f64 = 0.75;

/// Is this HTML? The server's own `Content-Type` is authoritative when it
/// says so; otherwise fall back to TinyJuice's content detection, which
/// already distinguishes HTML from JSON, diffs and code.
fn is_html(body: &str, content_type: Option<&str>) -> bool {
    if let Some(ct) = content_type {
        let ct = ct.to_ascii_lowercase();
        let mime = ct.split(';').next().unwrap_or("").trim().to_string();
        // An explicit non-HTML type is a statement, not a guess: a JSON API
        // that happens to embed markup must come back verbatim.
        if !mime.is_empty() && mime != "text/html" && mime != "application/xhtml+xml" {
            return false;
        }
        if !mime.is_empty() {
            return true;
        }
    }
    matches!(
        tinyjuice::detect_content_kind(body, &tinyjuice::types::ContentHint::default()),
        tinyjuice::types::ContentKind::Html
    )
}

/// Keep the head and the tail, drop the middle, and report how much went.
///
/// Returns the window and the number of chars elided. Cuts land on char
/// boundaries, and on a line boundary where one is close by, so the model
/// never sees a half-word or a half-line.
fn head_tail_window(content: &str, budget: usize) -> (String, usize) {
    if content.chars().count() <= budget {
        return (content.to_string(), 0);
    }
    let head_budget = (budget as f64 * HEAD_FRACTION) as usize;
    let tail_budget = budget.saturating_sub(head_budget);

    let head_end = crate::util::floor_char_boundary(content, head_budget);
    let head = &content[..head_end];
    // Snap back to a line break when one is within the last quarter of the
    // head, so the cut falls between paragraphs rather than mid-sentence.
    let head = match head.rfind('\n') {
        Some(nl) if nl > head_budget * 3 / 4 => &head[..nl],
        _ => head,
    };

    let tail_start = crate::util::floor_char_boundary(
        content,
        content.len().saturating_sub(tail_budget),
    );
    let tail = &content[tail_start..];
    let tail = match tail.find('\n') {
        Some(nl) if nl < tail_budget / 4 => &tail[nl + 1..],
        _ => tail,
    };

    let elided = content.len().saturating_sub(head.len() + tail.len());
    (format!("{head}\n\n[…]\n\n{tail}"), elided)
}

#[cfg(test)]
#[path = "web_fetch_tests.rs"]
mod tests;
