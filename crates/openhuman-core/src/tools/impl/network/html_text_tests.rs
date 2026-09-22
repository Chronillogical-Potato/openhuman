//! Behavior tests for the `web_fetch` HTML→text extractor.

use super::*;

#[test]
fn script_and_style_contents_are_dropped_entirely() {
    let html = r#"<html><head><style>body{color:red}</style></head>
        <body><script>var x = 1 < 2 && 3 > 2;</script><p>Hello</p></body></html>"#;
    let text = html_to_text(html);
    assert_eq!(text, "Hello");
    assert!(!text.contains("color"), "style body leaked: {text}");
    assert!(!text.contains("var x"), "script body leaked: {text}");
}

#[test]
fn block_elements_become_line_breaks_so_headings_dont_run_into_prose() {
    let html = "<h1>Title</h1><p>First para.</p><p>Second para.</p>";
    assert_eq!(html_to_text(html), "Title\nFirst para.\nSecond para.");
}

#[test]
fn inline_elements_do_not_break_a_sentence() {
    let html = "<p>The <b>quick</b> brown <a href=\"/x\">fox</a>.</p>";
    assert_eq!(html_to_text(html), "The quick brown fox.");
}

#[test]
fn entities_are_decoded_including_numeric_forms() {
    let html = "<p>A &amp; B &lt;tag&gt; &#39;quoted&#39; &#x2014; &nbsp;done&hellip;</p>";
    assert_eq!(html_to_text(html), "A & B <tag> 'quoted' — done…");
}

#[test]
fn an_unknown_entity_is_left_alone_rather_than_swallowed() {
    assert_eq!(html_to_text("<p>a &zzz; b</p>"), "a &zzz; b");
}

#[test]
fn comments_and_doctype_vanish() {
    let html = "<!doctype html><!-- hidden note --><p>Visible</p>";
    assert_eq!(html_to_text(html), "Visible");
}

#[test]
fn runs_of_whitespace_and_blank_lines_collapse() {
    let html = "<div>a     b</div><div></div><div></div><div>c</div>";
    assert_eq!(html_to_text(html), "a b\nc");
}

#[test]
fn an_unterminated_script_swallows_the_rest_like_a_browser_would() {
    // Malformed markup must not emit the script body as prose.
    let text = html_to_text("<p>before</p><script>junk(); more junk");
    assert_eq!(text, "before");
}

#[test]
fn an_unterminated_angle_bracket_is_kept_as_text_not_dropped() {
    let text = html_to_text("<p>done</p>5 < 6 and counting");
    assert!(text.contains("5 < 6 and counting"), "tail lost: {text}");
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

    let text = html_to_text(&html);
    assert!(
        text.len() * 10 < html.len(),
        "expected >10x shrink, got {} -> {}",
        html.len(),
        text.len()
    );
    assert!(text.contains("Real sentence of prose."));
    assert!(!text.contains("return"));
}

#[test]
fn html_is_detected_by_doctype_root_or_tag_density() {
    assert!(looks_like_html("<!DOCTYPE html><html><body>hi</body></html>"));
    assert!(looks_like_html("<div><p>one</p><span>two</span><a href=\"#\">three</a></div>"));
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
    let body = "あ".repeat(4096);
    assert!(!looks_like_html(&body));
}
