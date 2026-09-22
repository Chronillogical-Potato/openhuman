//! HTML → readable text for `web_fetch`.
//!
//! `web_fetch` used to hand the model (and, worse, the payload
//! summarizer) the raw response body: minified JS, inline CSS, SVG path
//! data and nav chrome included. A wire capture of one research turn
//! showed seven summarizer calls at 68k–197k prompt tokens each and
//! 1,083,069 input tokens for the turn, nearly all of it markup the
//! model had no use for.
//!
//! Reference agents don't pay that. Hermes' `web_extract` delegates
//! extraction to a vendor and budgets 15k chars of *clean markdown*;
//! Codex never fetches a page client-side at all. The cheap half of the
//! gap is extraction: dropping markup typically takes a page an order of
//! magnitude under the summarizer's 16 KB trigger, so the expensive path
//! stops firing rather than being made faster.
//!
//! This is deliberately a small hand-rolled scanner, not a parser. The
//! tree shape doesn't matter here — we only need "which bytes are text a
//! reader would see". A real HTML5 parser (`html5ever` and its
//! `markup5ever`/`string_cache` retinue) is a large dependency for a
//! crate that ships none today, and the repo actively sheds
//! dependencies (`scripts/assert-shed.sh`). Malformed markup degrades to
//! "some tags show up as text", which is the same failure the raw body
//! already had.

/// Elements whose *content* is machinery, not prose. Everything between
/// the open and close tag is dropped, including nested markup.
const DROPPED_ELEMENTS: &[&str] = &[
    "script", "style", "noscript", "template", "svg", "canvas", "head", "iframe", "object",
    "embed", "math",
];

/// Elements that imply a line break when they open or close, so that
/// stripping tags doesn't run a heading into the paragraph after it.
const BLOCK_ELEMENTS: &[&str] = &[
    "address", "article", "aside", "blockquote", "br", "dd", "div", "dl", "dt", "fieldset",
    "figcaption", "figure", "footer", "form", "h1", "h2", "h3", "h4", "h5", "h6", "header", "hr",
    "li", "main", "nav", "ol", "p", "pre", "section", "table", "td", "th", "tr", "ul",
];

/// Does this look like HTML at all? Cheap sniff over the first few KB:
/// a doctype, an `<html`/`<head`/`<body` root, or simply enough
/// tag-shaped runs to outweigh coincidence in prose or JSON.
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
    // Fall back to counting: a JSON document quoting a `<b>` shouldn't
    // trip this, but a fragment of real markup should.
    let tagish = ["<div", "<p>", "<p ", "<span", "<a ", "<table", "<li>", "<script", "<meta"]
        .iter()
        .filter(|needle| lower.contains(*needle))
        .count();
    tagish >= 3
}

/// Strip markup and return the text a reader would see.
///
/// Block-level elements become newlines; `DROPPED_ELEMENTS` and their
/// contents vanish; comments and doctypes vanish; entities are decoded;
/// runs of whitespace collapse. The result keeps paragraph structure
/// (blank line between blocks) because that is what makes the text
/// readable — and chunkable — downstream.
pub fn html_to_text(body: &str) -> String {
    let bytes = body.as_bytes();
    let mut out = String::with_capacity(body.len() / 4);
    let mut i = 0usize;

    while i < bytes.len() {
        if bytes[i] != b'<' {
            let start = i;
            while i < bytes.len() && bytes[i] != b'<' {
                i += 1;
            }
            push_text(&mut out, &body[start..i]);
            continue;
        }

        // `<!-- comment -->`, `<!doctype …>`, `<![CDATA[…]]>`
        if body[i..].starts_with("<!--") {
            i = find_after(body, i + 4, "-->").unwrap_or(bytes.len());
            continue;
        }
        if body[i..].starts_with("<!") {
            i = find_after(body, i + 2, ">").unwrap_or(bytes.len());
            continue;
        }

        let Some(tag_end) = find_after(body, i + 1, ">") else {
            // Unterminated `<` — treat the remainder as text rather than
            // silently dropping the tail of the document.
            push_text(&mut out, &body[i..]);
            break;
        };
        let inner = &body[i + 1..tag_end - 1];
        let closing = inner.starts_with('/');
        let name = element_name(inner);

        if !closing && DROPPED_ELEMENTS.contains(&name.as_str()) && !inner.ends_with('/') {
            // Skip to the matching close tag. An unterminated <script>
            // swallowing the rest of the document is the correct read of
            // malformed markup — the browser does the same.
            match find_close_tag(body, tag_end, &name) {
                Some(after) => i = after,
                None => break,
            }
            continue;
        }

        if BLOCK_ELEMENTS.contains(&name.as_str()) {
            push_break(&mut out);
        }
        i = tag_end;
    }

    normalize(&out)
}

/// Lowercased element name from the inside of a tag (`/p class=x` → `p`).
fn element_name(inner: &str) -> String {
    inner
        .trim_start_matches('/')
        .split(|c: char| c.is_whitespace() || c == '/' || c == '>')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Index just past `needle`, searching from `from`.
fn find_after(hay: &str, from: usize, needle: &str) -> Option<usize> {
    hay.get(from..)
        .and_then(|rest| rest.find(needle).map(|at| from + at + needle.len()))
}

/// Index just past `</name …>`, searching case-insensitively from `from`.
fn find_close_tag(hay: &str, from: usize, name: &str) -> Option<usize> {
    let rest = hay.get(from..)?;
    let lower = rest.to_ascii_lowercase();
    let needle = format!("</{name}");
    let at = lower.find(&needle)?;
    find_after(hay, from + at + needle.len(), ">")
}

fn push_text(out: &mut String, raw: &str) {
    if raw.is_empty() {
        return;
    }
    out.push_str(&decode_entities(raw));
}

fn push_break(out: &mut String) {
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
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
        let Some(semi) = rest[..rest.len().min(12)].find(';') else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" | "#160" => Some(' '),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            "hellip" => Some('…'),
            "rsquo" => Some('\''),
            "lsquo" => Some('\''),
            "ldquo" => Some('"'),
            "rdquo" => Some('"'),
            _ => numeric_entity(entity),
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

/// Collapse the whitespace that markup leaves behind: spaces within a
/// line, and runs of blank lines down to one.
fn normalize(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut blank_run = 0usize;
    for line in raw.lines() {
        let mut collapsed = String::with_capacity(line.len());
        let mut in_space = false;
        for ch in line.chars() {
            if ch.is_whitespace() {
                in_space = true;
                continue;
            }
            if in_space && !collapsed.is_empty() {
                collapsed.push(' ');
            }
            in_space = false;
            collapsed.push(ch);
        }
        if collapsed.is_empty() {
            blank_run += 1;
            if blank_run > 1 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(&collapsed);
        out.push('\n');
    }
    out.trim().to_string()
}

#[cfg(test)]
#[path = "html_text_tests.rs"]
mod tests;
