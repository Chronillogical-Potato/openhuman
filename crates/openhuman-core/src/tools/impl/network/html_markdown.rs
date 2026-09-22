//! HTML → Markdown for `web_fetch`.
//!
//! `web_fetch` used to hand the model — and, worse, the payload
//! summarizer — the raw response body: minified JS, inline CSS, SVG path
//! data and nav chrome included. A wire capture of one research turn
//! showed seven summarizer calls at 68k–197k prompt tokens each and
//! 1,083,069 input tokens for the turn, nearly all of it markup the
//! model had no use for.
//!
//! Reference agents don't pay that. Hermes' `web_extract` budgets 15k
//! chars of *clean markdown* (its vendors do the extraction); Codex
//! never fetches a page client-side at all. Extraction is the cheap half
//! of the gap: dropping markup typically takes a page an order of
//! magnitude under the summarizer's 16 KB trigger, so the expensive path
//! stops firing rather than being made faster.
//!
//! Markdown rather than flat text because the structure is the part the
//! model acts on: headings tell it which section answers the question,
//! and link targets are what it feeds the next `web_fetch`. A text
//! extractor throws both away.
//!
//! ## Why this isn't TinyJuice's extractor
//!
//! `vendor/tinyjuice/src/compressors/html.rs` does something close, but
//! it is (a) html→text, keeping no headings or hrefs, and (b) behind the
//! loadable-module boundary: OpenHuman links only `tinyjuice-bus`, and
//! `inference/tokenjuice/README.md` forbids adding the engine crate
//! back. Reaching it needs the `modules` feature, a loaded cdylib and
//! `compaction_enabled` (default `false`). A core tool's default output
//! must not depend on whether an optional module loaded, so extraction
//! lives here. If the two ever need to agree, this is the host-side one.
//!
//! This is a scanner, not a parser: the tree shape doesn't matter, only
//! "which bytes would a reader see, and what role did they play".
//! `html5ever` and its `markup5ever`/`string_cache` retinue is a large
//! dependency for a crate that ships no HTML parser today, in a repo
//! that actively sheds dependencies (`scripts/assert-shed.sh`).
//! Malformed markup degrades to "some tags show up as text" — the same
//! failure the raw body already had.

/// Elements whose *content* is machinery, not prose. Everything up to
/// the matching close tag is dropped, nested markup included.
const DROPPED_ELEMENTS: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "canvas", "iframe", "object", "embed",
    "math", "form", "select",
];

/// Elements whose body is taken verbatim and re-emitted as a fenced
/// block, so indentation and line breaks in code survive.
const VERBATIM_ELEMENTS: &[&str] = &["pre"];

/// Elements that force a line break when they open or close, so that
/// stripping tags doesn't run a heading into the paragraph after it.
const BLOCK_ELEMENTS: &[&str] = &[
    "address", "article", "aside", "blockquote", "dd", "div", "dl", "dt", "fieldset", "figcaption",
    "figure", "footer", "header", "hr", "main", "nav", "ol", "p", "section", "table", "tbody",
    "td", "tfoot", "th", "thead", "tr", "ul",
];

/// Upper bound on a single extracted link target. A tracking URL can run
/// to several KB of query string, which is pure cost in the transcript.
const MAX_HREF_CHARS: usize = 300;

/// Does this look like HTML at all? Cheap sniff over the first few KB:
/// a doctype, an `<html`/`<head`/`<body` root, or enough tag-shaped runs
/// to outweigh coincidence in prose or JSON.
pub fn looks_like_html(body: &str) -> bool {
    let head = &body[..crate::util::floor_char_boundary(body, 4096)];
    let lower = head.to_ascii_lowercase();
    if lower.contains("<!doctype html")
        || lower.contains("<html")
        || lower.contains("<head")
        || lower.contains("<body")
    {
        return true;
    }
    // A JSON document quoting "<b>" shouldn't trip this; a fragment of
    // real markup should.
    ["<div", "<p>", "<p ", "<span", "<a ", "<table", "<li>", "<script", "<meta"]
        .iter()
        .filter(|needle| lower.contains(*needle))
        .count()
        >= 3
}

/// Convert an HTML document to Markdown.
pub fn html_to_markdown(body: &str) -> String {
    Converter::new(body).run()
}

struct Converter<'a> {
    body: &'a str,
    out: String,
    /// Open `<a>` elements: where their text began in `out`, and the
    /// href to wrap it with on close.
    links: Vec<(usize, String)>,
    /// Nesting depth of ordered/unordered lists, for indenting `-`.
    list_depth: usize,
}

impl<'a> Converter<'a> {
    fn new(body: &'a str) -> Self {
        Self {
            body,
            out: String::with_capacity(body.len() / 4),
            links: Vec::new(),
            list_depth: 0,
        }
    }

    fn run(mut self) -> String {
        let bytes = self.body.as_bytes();
        let mut i = 0usize;

        while i < bytes.len() {
            if bytes[i] != b'<' {
                let start = i;
                while i < bytes.len() && bytes[i] != b'<' {
                    i += 1;
                }
                let text = decode_entities(&self.body[start..i]);
                self.out.push_str(&text);
                continue;
            }

            // `<!-- comment -->`, `<!doctype …>`, `<![CDATA[…]]>`
            if self.body[i..].starts_with("<!--") {
                i = find_after(self.body, i + 4, "-->").unwrap_or(bytes.len());
                continue;
            }
            if self.body[i..].starts_with("<!") {
                i = find_after(self.body, i + 2, ">").unwrap_or(bytes.len());
                continue;
            }

            let Some(tag_end) = find_after(self.body, i + 1, ">") else {
                // Unterminated `<` — the rest is text, not a dropped tail.
                let text = decode_entities(&self.body[i..]);
                self.out.push_str(&text);
                break;
            };
            let inner = &self.body[i + 1..tag_end - 1];
            let closing = inner.starts_with('/');
            let self_closing = inner.ends_with('/');
            let name = element_name(inner);

            if !closing && !self_closing && DROPPED_ELEMENTS.contains(&name.as_str()) {
                // An unterminated <script> swallowing the remainder is
                // the correct read of malformed markup; a browser agrees.
                match find_close_tag(self.body, tag_end, &name) {
                    Some(after) => i = after,
                    None => break,
                }
                continue;
            }

            if !closing && !self_closing && VERBATIM_ELEMENTS.contains(&name.as_str()) {
                i = self.emit_verbatim(tag_end, &name).unwrap_or(bytes.len());
                continue;
            }

            self.emit_tag(&name, inner, closing);
            i = tag_end;
        }

        normalize(&self.out)
    }

    /// `<pre>` and friends: take the body as-is, strip any inner tags,
    /// and fence it so whitespace survives the normalizer.
    fn emit_verbatim(&mut self, from: usize, name: &str) -> Option<usize> {
        let close = find_close_tag_start(self.body, from, name)?;
        let raw = &self.body[from..close];
        let text = decode_entities(&strip_tags(raw));
        self.break_line();
        self.out.push_str("```\n");
        self.out.push_str(text.trim_matches('\n'));
        self.out.push_str("\n```");
        self.break_line();
        find_after(self.body, close, ">")
    }

    fn emit_tag(&mut self, name: &str, inner: &str, closing: bool) {
        match name {
            "br" => self.out.push('\n'),
            "title" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.break_line();
                if !closing {
                    let level = match name {
                        "title" | "h1" => 1,
                        other => other[1..].parse::<usize>().unwrap_or(1),
                    };
                    self.out.push_str(&"#".repeat(level));
                    self.out.push(' ');
                }
            }
            "li" => {
                if !closing {
                    self.break_line();
                    let indent = "  ".repeat(self.list_depth.saturating_sub(1));
                    self.out.push_str(&indent);
                    self.out.push_str("- ");
                }
            }
            "ul" | "ol" => {
                if closing {
                    self.list_depth = self.list_depth.saturating_sub(1);
                } else {
                    self.list_depth += 1;
                }
                self.break_line();
            }
            "b" | "strong" => self.push_emphasis("**"),
            "i" | "em" => self.push_emphasis("*"),
            "code" => self.push_emphasis("`"),
            "a" => {
                if closing {
                    self.close_link();
                } else if let Some(href) = attribute(inner, "href") {
                    self.links.push((self.out.len(), href));
                }
            }
            "img" => {
                if let Some(alt) = attribute(inner, "alt").filter(|a| !a.trim().is_empty()) {
                    // Keep the caption, drop the source: an `img` src is
                    // either a URL the model can't read or, worse, a
                    // multi-KB base64 data: blob.
                    self.out.push_str(&format!("[IMAGE: {}]", alt.trim()));
                }
            }
            other if BLOCK_ELEMENTS.contains(&other) => self.break_line(),
            _ => {}
        }
    }

    /// Emphasis markers are only worth emitting around real text; an
    /// empty `<b></b>` would otherwise leave `****` behind.
    fn push_emphasis(&mut self, marker: &str) {
        if self.out.ends_with(marker) {
            self.out.truncate(self.out.len() - marker.len());
            return;
        }
        self.out.push_str(marker);
    }

    fn close_link(&mut self) {
        let Some((start, href)) = self.links.pop() else {
            return;
        };
        if start > self.out.len() {
            return;
        }
        let text = self.out[start..].trim().to_string();
        // A link with no text, or one pointing at a fragment/javascript
        // handler, is navigation chrome — keep the words, drop the wrap.
        if text.is_empty() || !is_useful_href(&href) {
            return;
        }
        self.out.truncate(start);
        self.out.push_str(&format!("[{text}]({href})"));
    }

    fn break_line(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }
}

fn is_useful_href(href: &str) -> bool {
    let h = href.trim();
    !(h.is_empty()
        || h.starts_with('#')
        || h.len() > MAX_HREF_CHARS
        || h.to_ascii_lowercase().starts_with("javascript:")
        || h.to_ascii_lowercase().starts_with("data:"))
}

/// Lowercased element name from inside a tag (`/p class=x` → `p`).
fn element_name(inner: &str) -> String {
    inner
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Pull a quoted attribute value out of a tag's interior.
fn attribute(inner: &str, name: &str) -> Option<String> {
    let lower = inner.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower[from..].find(name) {
        let at = from + at;
        // Must be preceded by whitespace and followed by `=`, so `href`
        // doesn't match inside `data-href` or a text run.
        let before_ok = at == 0 || inner[..at].ends_with(char::is_whitespace);
        let rest = inner[at + name.len()..].trim_start();
        if before_ok && rest.starts_with('=') {
            let value = rest[1..].trim_start();
            let quoted = value.strip_prefix('"').map(|v| (v, '"')).or_else(|| {
                value
                    .strip_prefix('\'')
                    .map(|v| (v, '\''))
            });
            return Some(match quoted {
                Some((v, q)) => decode_entities(v.split(q).next().unwrap_or("")),
                None => decode_entities(
                    value
                        .split(|c: char| c.is_whitespace() || c == '>')
                        .next()
                        .unwrap_or(""),
                ),
            });
        }
        from = at + name.len();
    }
    None
}

fn strip_tags(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut depth = 0usize;
    for ch in raw.chars() {
        match ch {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn find_after(hay: &str, from: usize, needle: &str) -> Option<usize> {
    hay.get(from..)
        .and_then(|rest| rest.find(needle).map(|at| from + at + needle.len()))
}

/// Byte index of the `<` that opens `</name`, searching case-insensitively.
fn find_close_tag_start(hay: &str, from: usize, name: &str) -> Option<usize> {
    let rest = hay.get(from..)?;
    let lower = rest.to_ascii_lowercase();
    lower.find(&format!("</{name}")).map(|at| from + at)
}

/// Byte index just past `</name …>`.
fn find_close_tag(hay: &str, from: usize, name: &str) -> Option<usize> {
    let start = find_close_tag_start(hay, from, name)?;
    find_after(hay, start, ">")
}

/// The entities that actually show up in prose. A full named-entity
/// table is a dependency's worth of data for diminishing returns; the
/// numeric forms cover the long tail.
fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let window = &rest[..rest.len().min(12)];
        let Some(semi) = window.find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let decoded = match &rest[1..semi] {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            "rsquo" | "lsquo" => Some('\''),
            "ldquo" | "rdquo" => Some('"'),
            other => numeric_entity(other),
        };
        match decoded {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn numeric_entity(entity: &str) -> Option<char> {
    let digits = entity.strip_prefix('#')?;
    let code = match digits.strip_prefix(['x', 'X']) {
        Some(hex) => u32::from_str_radix(hex, 16).ok()?,
        None => digits.parse::<u32>().ok()?,
    };
    char::from_u32(code)
}

/// Collapse the whitespace markup leaves behind: runs of spaces within a
/// line, and runs of blank lines down to one. Fenced blocks are passed
/// through untouched so code keeps its indentation.
fn normalize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank_run = 0usize;
    let mut in_fence = false;

    for line in raw.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            out.push_str(line.trim_end());
            out.push('\n');
            blank_run = 0;
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push('\n');
            continue;
        }

        let mut collapsed = String::with_capacity(line.len());
        let mut pending_space = false;
        for ch in line.chars() {
            if ch.is_whitespace() {
                pending_space = true;
                continue;
            }
            if pending_space && !collapsed.is_empty() {
                collapsed.push(' ');
            }
            pending_space = false;
            collapsed.push(ch);
        }

        // A list bullet or heading marker with nothing after it is a
        // leftover from stripped markup, not content.
        let trimmed = collapsed.trim_end();
        if trimmed == "-" || trimmed.chars().all(|c| c == '#') && !trimmed.is_empty() {
            continue;
        }

        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(trimmed);
        out.push('\n');
    }
    out.trim().to_string()
}

#[cfg(test)]
#[path = "html_markdown_tests.rs"]
mod tests;
