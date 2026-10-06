use super::*;

#[test]
fn remote_media_survives_durable_roundtrip_with_its_modality_and_mime() {
    for (kind, mime) in [
        ("audio", "audio/mpeg"),
        ("video", "video/mp4"),
        ("document", "application/pdf"),
    ] {
        let url = "https://example.invalid/upload";
        let media = MediaRef::Url {
            url: url.into(),
            media_type: Some(mime.into()),
        };
        let part = part_from_media(&media, kind).expect("remote reference is durable");
        let (source, stored_mime) = match (&part, kind) {
            (TranscriptPart::Audio { source, mime_type }, "audio")
            | (TranscriptPart::Video { source, mime_type }, "video")
            | (TranscriptPart::Document { source, mime_type }, "document") => (source, mime_type),
            _ => panic!("durable reference lost its modality: {part:?}"),
        };
        assert_eq!(stored_mime, mime);
        assert_eq!(media_from_part(source, stored_mime), media);
        let serialized = serde_json::to_string(&part).unwrap();
        let restored: TranscriptPart = serde_json::from_str(&serialized).unwrap();
        assert_eq!(restored, part);
    }
}

#[test]
fn untyped_workspace_media_keeps_its_path_and_defaults_to_binary_document() {
    let path = "uploads/archive.zip";
    let media = MediaRef::Path {
        path: path.into(),
        media_type: None,
    };
    let part = part_from_media(&media, "unknown").unwrap();
    assert_eq!(
        part,
        TranscriptPart::Document {
            source: TranscriptMediaRef::Path { path: path.into() },
            mime_type: "application/octet-stream".into(),
        }
    );
}

#[test]
fn inline_provider_payloads_cannot_become_durable_media_parts() {
    for kind in ["audio", "video", "document"] {
        let media = MediaRef::base64("cHJpdmF0ZQ==", "application/octet-stream");
        assert!(part_from_media(&media, kind).is_none());
    }
}
