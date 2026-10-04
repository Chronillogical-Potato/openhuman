//! Mechanical durable media conversions; provider payloads never become session parts.
use tinyagents_session::transcript::{TranscriptMediaRef, TranscriptPart};
use tinyinference_llm::message::MediaRef;

pub(crate) fn media_from_part(source: &TranscriptMediaRef, mime: &str) -> MediaRef {
    match source {
        TranscriptMediaRef::Path { path } => MediaRef::Path {
            path: path.clone(),
            media_type: Some(mime.into()),
        },
        TranscriptMediaRef::Url { url } => MediaRef::Url {
            url: url.clone(),
            media_type: Some(mime.into()),
        },
    }
}

pub(crate) fn part_from_media(media: &MediaRef, kind: &str) -> Option<TranscriptPart> {
    let (source, mime_type) = match media {
        MediaRef::Path { path, media_type } => (
            TranscriptMediaRef::Path { path: path.clone() },
            media_type.clone(),
        ),
        MediaRef::Url { url, media_type } => (
            TranscriptMediaRef::Url { url: url.clone() },
            media_type.clone(),
        ),
        // Inline payloads are provider-only and must never become durable rows.
        MediaRef::Base64 { .. } => return None,
    };
    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".into());
    Some(match kind {
        "audio" => TranscriptPart::Audio { source, mime_type },
        "video" => TranscriptPart::Video { source, mime_type },
        _ => TranscriptPart::Document { source, mime_type },
    })
}

#[cfg(test)]
#[path = "codec_tests.rs"]
mod tests;
