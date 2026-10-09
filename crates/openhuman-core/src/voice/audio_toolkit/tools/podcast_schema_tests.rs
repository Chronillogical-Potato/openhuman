use super::*;
use serde_json::json;

fn expected_audio_generate_and_email_podcast() -> serde_json::Value {
    json!({
            "type": "object",
            "required": ["text", "to", "subject", "body"],
            "properties": {
                "text": { "type": "string", "description": "Text to synthesize into audio." },
                "to": { "type": "string", "description": "Recipient email address." },
                "subject": { "type": "string", "description": "Email subject line." },
                "body": { "type": "string", "description": "Email body text." },
                "title": { "type": "string", "description": "Optional title used in the default file name." },
                "output_path": { "type": "string", "description": "Optional workspace-relative output path." },
                "provider": { "type": "string", "description": "Optional TTS provider override (`cloud` or `piper`)." },
                "voice": { "type": "string", "description": "Optional voice id for the chosen provider." },
                "format": { "type": "string", "enum": ["mp3", "wav"], "description": "Desired audio format." },
                "attachment_name": { "type": "string", "description": "Optional attachment file name override." }
            }
        })
}

#[test]
fn audio_generate_and_email_podcast_static_schema_matches_json_literal() {
    let tool = todo!("construct tool");
    assert_eq!(Tool::parameters_schema(&tool), expected_audio_generate_and_email_podcast());
}
