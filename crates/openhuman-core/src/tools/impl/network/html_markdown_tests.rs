//! Behavior tests for the `web_fetch` HTML→Markdown converter.

use super::*;

#[test]
fn script_and_style_contents_are_dropped_entirely() {
    let html = r#"<html><head><style>body{color:red}</style></head>
        <body><script>var x = 1 < 2 && 3 > 2;</script><p>Hello</p></body></html>"#;
    let md = html_to_markdown(html);
    assert_eq!(md, "Hello");
    assert!(!md.contains("color"), "style body leaked: {md}");
    assert!(!md.contains("var x"), "script body leaked: {md}");
}

#[test]
fn headings_become_atx_markers_at_their_level() {
    let html = "<h1>Title</h1><p>Intro.</p><h3>Detail</h3><p>Body.</p>";
    assert_eq!(html_to_markdown(html), "# Title\nIntro.\n### Detail\nBody.");
}

#[test]
fn the_document_title_is_kept_as_a_top_level_heading() {
    // Often the highest-signal string on the page.
    let html = "<html><head><title>Rust Docs</title></head><body><p>x</p></body></html>";
    assert!(html_to_markdown(html).starts_with("# Rust Docs"));
}

#[test]
fn links_keep_their_target_so_the_model_can_fetch_the_next_page() {
    let html = r#"<p>See <a href="https://example.com/spec">the spec</a> for more.</p>"#;
    assert_eq!(
        html_to_markdown(html),
        "See [the spec](https://example.com/spec) for more."
    );
}

#[test]
fn navigation_chrome_links_keep_their_words_but_drop_the_wrapper() {
    for href in ["#", "#section-2", "javascript:void(0)", "data:text/html,x"] {
        let html = format!(r#"<p>Go <a href="{href}">here</a> now.</p>"#);
        assert_eq!(html_to_markdown(&html), "Go here now.", "href={href}");
    }
}

#[test]
fn an_overlong_tracking_url_is_dropped_rather_than_paid_for() {
    let href = format!("https://e.com/?{}", "utm=x&".repeat(100));
    assert!(href.len() > MAX_HREF_CHARS);
    let html = format!(r#"<a href="{href}">click</a>"#);
    assert_eq!(html_to_markdown(&html), "click");
}

#[test]
fn list_items_become_bullets_and_nest_by_depth() {
    let html = "<ul><li>one</li><li>two<ul><li>inner</li></ul></li></ul>";
    assert_eq!(html_to_markdown(html), "- one\n- two\n  - inner");
}

#[test]
fn pre_blocks_are_fenced_and_keep_their_indentation() {
    let html = "<pre><code>fn main() {\n    println!(\"hi\");\n}</code></pre>";
    let md = html_to_markdown(html);
    assert_eq!(md, "```\nfn main() {\n    println!(\"hi\");\n}\n```");
}

#[test]
fn emphasis_survives_as_markdown() {
    let html = "<p><strong>bold</strong> and <em>italic</em> and <code>lit</code></p>";
    assert_eq!(html_to_markdown(html), "**bold** and *italic* and `lit`");
}

#[test]
fn images_keep_their_caption_and_never_their_base64_payload() {
    let html = r#"<p><img alt="A chart" src="data:image/png;base64,AAAAAAAAAAAAAAAA"></p>"#;
    let md = html_to_markdown(html);
    assert_eq!(md, "[IMAGE: A chart]");
    assert!(!md.contains("base64"), "data URI leaked: {md}");
}

#[test]
fn an_image_with_no_alt_text_contributes_nothing() {
    assert_eq!(html_to_markdown(r#"<p>a<img src="/x.png">b</p>"#), "ab");
}

#[test]
fn entities_are_decoded_including_numeric_and_hex_forms() {
    let html = "<p>A &amp; B &lt;tag&gt; &#39;q&#39; &#x2014; &nbsp;done&hellip;</p>";
    assert_eq!(html_to_markdown(html), "A & B <tag> 'q' — done…");
}

#[test]
fn an_unknown_entity_is_left_alone_rather_than_swallowed() {
    assert_eq!(html_to_markdown("<p>a &zzz; b</p>"), "a &zzz; b");
}

#[test]
fn comments_and_doctype_vanish() {
    assert_eq!(
        html_to_markdown("<!doctype html><!-- hidden --><p>Visible</p>"),
        "Visible"
    );
}

#[test]
fn an_unterminated_script_swallows_the_rest_like_a_browser_would() {
    assert_eq!(html_to_markdown("<p>before</p><script>junk(); more"), "before");
}

#[test]
fn an_unterminated_angle_bracket_is_kept_as_text_not_dropped() {
    let md = html_to_markdown("<p>done</p>5 < 6 and counting");
    assert!(md.contains("5 < 6 and counting"), "tail lost: {md}");
}

#[test]
fn a_data_href_attribute_is_not_mistaken_for_href() {
    let html = r#"<a data-href="/wrong" href="/right">t</a>"#;
    assert_eq!(html_to_markdown(html), "[t](/right)");
}

#[test]
fn unquoted_attribute_values_are_still_read() {
    assert_eq!(html_to_markdown("<a href=/plain>t</a>"), "[t](/plain)");
}

#[test]
fn extraction_shrinks_a_markup_heavy_page_by_an_order_of_magnitude() {
    // The property that matters for cost: a page whose bytes are mostly
    // machinery must come out near the size of its prose.
    let mut html = String::from("<html><head>");
    for i in 0..200 {
        html.push_str(&format!("<script>function f{i}(){{return {i}*2;}}</script>"));
    }
    html.push_str("</head><body>");
    for _ in 0..10 {
        html.push_str("<div class=\"a b c\"><span>Real sentence of prose.</span></div>");
    }
    html.push_str("</body></html>");

    let md = html_to_markdown(&html);
    assert!(
        md.len() * 10 < html.len(),
        "expected >10x shrink, got {} -> {}",
        html.len(),
        md.len()
    );
    assert!(md.contains("Real sentence of prose."));
    assert!(!md.contains("return"));
}

#[test]
fn html_is_detected_by_doctype_root_or_tag_density() {
    assert!(looks_like_html("<!DOCTYPE html><html><body>hi</body></html>"));
    assert!(looks_like_html(
        "<div><p>one</p><span>two</span><a href=\"#\">three</a></div>"
    ));
}

#[test]
fn json_and_plain_text_are_not_mistaken_for_html() {
    assert!(!looks_like_html(
        r#"{"note": "use <b> for bold", "n": 1, "ok": true}"#
    ));
    assert!(!looks_like_html(
        "# A markdown README\n\nSome prose about x < y comparisons.\n"
    ));
}

#[test]
fn a_multibyte_document_does_not_panic_on_the_sniff_boundary() {
    // `looks_like_html` slices the first 4 KB; the cut must land on a
    // char boundary even when the padding is multibyte.
    assert!(!looks_like_html(&"あ".repeat(4096)));
}
