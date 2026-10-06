use super::*;

fn marker(name: &str) -> String {
    super::super::attachments::Attachment {
        path: "uploads/thread/id/file.zip".into(),
        name: name.into(),
        mime: "application/zip".into(),
        size_bytes: 3,
    }
    .marker()
}

#[test]
fn raw_and_staged_names_match_without_decoding_payloads() {
    let raw = "[FILE:data:application/zip;name=Report+%CE%B1.zip;base64,opaque-invalid-payload]";
    assert_eq!(text_preview(raw), "Report α.zip");
    assert_eq!(text_preview(raw), text_preview(&marker("Report α.zip")));
    assert_eq!(text_preview("[IMAGE:/local/file.png]"), "attachment");
    assert_eq!(
        text_preview("[FILE:data:audio/mpeg;base64,secret]"),
        "attachment"
    );
}

#[test]
fn captions_match_after_staging_and_preserve_paragraphs() {
    let raw =
        "  Check   this [FILE:data:application/zip;name=a.zip;base64,secret]\n\nnext paragraph  ";
    let staged = format!("  Check   this {}\n\nnext paragraph  ", marker("a.zip"));
    assert_eq!(text_preview(raw), "Check this \n\nnext paragraph");
    assert_eq!(text_preview(raw), text_preview(&staged));
}

#[test]
fn mixed_markers_keep_source_order() {
    let mixed = format!(
        "[IMAGE:data:image/png;name=first.png;base64,secret] {} [FILE:data:audio/mpeg;name=third.mp3;base64,secret]",
        marker("second α.zip")
    );
    assert_eq!(text_preview(&mixed), "first.png, second α.zip, third.mp3");
}

#[test]
fn invalid_durable_reference_stays_literal() {
    let invalid = super::super::attachments::Attachment {
        path: "../escape.zip".into(),
        name: "escape.zip".into(),
        mime: "application/zip".into(),
        size_bytes: 3,
    }
    .marker();
    assert_eq!(
        text_preview(&invalid),
        crate::core::events::clip_to_chars(&invalid, 80)
    );
    assert_eq!(text_preview("[ATTACHMENT:invalid]"), "[ATTACHMENT:invalid]");
}

#[test]
fn clips_unicode_codepoints_for_captions_and_names() {
    let long = "🦀".repeat(81);
    let expected = format!("{}…", "🦀".repeat(80));
    assert_eq!(text_preview(&long), expected);
    assert_eq!(text_preview(&marker(&long)), expected);
    assert_eq!(text_preview(&"α".repeat(80)), "α".repeat(80));
}
