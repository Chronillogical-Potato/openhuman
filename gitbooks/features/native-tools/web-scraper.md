---
description: >-
  The `web_fetch` tool: a single-purpose GET-and-read primitive that returns a
  page as Markdown rather than raw HTML, behind an allowlist and an SSRF guard.
icon: globe
---

# Web Scraper

`web_fetch` is the agent's GET-and-read primitive, deliberately separate from `http_request` and `curl`. It exists because the agent rarely wants the markup: it wants the page. HTML comes back converted to Markdown, with links kept and scripts dropped.

Reach for `http_request` instead when you need POST, custom headers or richer HTTP semantics, and for `curl` when the point is to stream a file to disk.

---

## Arguments

| Argument | Required | Effect |
| --- | --- | --- |
| `url` | Yes | An absolute `http` or `https` URL. |
| `max_bytes` | No | Truncate the body at this many bytes instead of the configured cap. |
| `raw` | No | Skip the Markdown conversion and return the body as served. It is not a way around the limits: the response cap and any `max_bytes` still truncate, and the result still carries the truncation marker. |
| `summary_focus` | No | An OpenHuman addition that steers the [TokenJuice](../token-compression.md) summary toward a topic. |

The tool is read-only and never prompts for approval. Parallel `web_fetch` calls are safe, since a GET is idempotent.

Conversion is driven by the response `Content-Type`, falling back to content sniffing when the header is absent. A response that is explicitly not HTML is returned verbatim.

---

## Limits

| Limit | Value | Where it comes from |
| --- | --- | --- |
| Body cap | 1,000,000 bytes | `[http_request].max_response_size` |
| Request timeout | 30 seconds | `[http_request].timeout_secs` |
| Text handed to the model in one result | 24,000 characters | `max_result_size_chars` in `web_fetch.rs` |
| Redirects followed | None | `redirect::Policy::none()` |

The first two are the shipped defaults in `crates/openhuman-core/src/config/schema/tools/http.rs` and are shared with `http_request`. A `0` in either key is treated as unset and falls back to the default, so a stale zero cannot disable every fetch.

Two behaviours are worth knowing:

- **Oversize pages are truncated, not dropped.** The body is cut at a character boundary, never mid-character, and the result header records `download_capped_at=<N>B` so the agent knows it is looking at a partial page. The cap bounds what reaches the model rather than what is downloaded, since the response is read before it is trimmed. Streaming with a hard download ceiling is `curl`'s job, not this tool's.
- **Past 24,000 characters the remainder is not discarded.** The full extracted page is spilled to an artifact and the result carries the `file_read` call that pages through it.

**Redirects are never followed.** A redirect target can sit on a host the allowlist would refuse, so a 3xx returns as a result naming its `Location` and leaves the decision to the agent. Re-calling `web_fetch` with that URL runs the whole guard again from scratch rather than inheriting the first call's approval.

---

## The allowlist and the URL guard

Every fetch passes `tinytools-std`'s URL guard (`vendor/tinyagents/vendor/tinytools/crates/tinytools-std/src/url_guard/`) before any connection is made. It is the same gate `http_request` and `curl` use.

**Shape.** Only `http` and `https` are accepted. URLs carrying whitespace, a backslash, userinfo (`user:pass@`), a percent-encoded authority, or an IPv6 literal host are rejected outright.

**Allowed websites.** `[http_request].allowed_domains` ships as `["*"]`, so research works out of the box. An exact host also matches its subdomains, `"*"` allows all public sites, and an empty list blocks all web access. If every entry is malformed the list fails closed rather than open. Narrow it from the allowed-websites setting described in [Web Search](web-search.md#settings), which writes the same key.

**Address checks.** The guard resolves the hostname and refuses the fetch if any resolved address is non-global: loopback, RFC1918 private, link-local (which covers the cloud metadata address `169.254.169.254`), CGNAT, the unspecified and broadcast ranges, and the IPv6 equivalents including unique-local, link-local and IPv4-mapped forms. Hostnames such as `localhost`, anything ending in `.localhost`, and anything under a `.local` TLD are refused by name. These checks apply even under `"*"`, so the wildcard opens public hosts and never the private network.

Under privacy mode's local-only setting the fetch is refused before validation or DNS, so a blocked destination is never even looked up. Rate limiting applies, and the egress disclosure spine records the destination host before the request leaves.

---

## What it's good for

- Reading articles, documentation pages and GitHub READMEs without the surrounding noise.
- Following up on a [Web Search](web-search.md) result.
- Summarising a single known page on demand.

---

## See also

- [Web Search](web-search.md): finds the URLs to feed this tool, and owns the allowed-websites list.
- [Smart Token Compression](../token-compression.md): what trims a long page before it reaches the model.
- [Browser & Computer Control](browser-and-computer.md): for pages that need clicking rather than reading.
- [Privacy & Security](../privacy-and-security.md): the local-only mode and egress disclosure behind the guard.
