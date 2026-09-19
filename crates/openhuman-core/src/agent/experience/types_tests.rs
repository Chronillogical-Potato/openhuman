use super::*;

fn redact_text_masks_secret_like_values() {
    let redacted = redact_text("token=abc123 password: hunter2 normal");
    assert!(redacted.contains("token=[redacted]"));
    assert!(redacted.contains("password: [redacted]"));
    assert!(!redacted.contains("abc123"));
    assert!(!redacted.contains("hunter2"));
    assert!(redacted.contains("normal"));
}

fn redact_text_masks_bearer_tokens_and_openai_style_keys() {
    let redacted =
        redact_text("Authorization: Bearer secret-token sk-abcdefghijklmnopqrstuvwxyz123456");
    assert!(!redacted.contains("secret-token"));
    assert!(!redacted.contains("sk-abcdefghijklmnopqrstuvwxyz123456"));
    assert!(redacted.contains("Bearer [redacted]"));
    assert!(redacted.contains("sk-[redacted]"));
}

fn stable_experience_id_is_repeatable() {
    let sequence = vec!["grep".to_string(), "file_read".to_string()];
    let first = stable_experience_id("same task", &sequence, ExperienceOutcome::Success);
    let second = stable_experience_id("same task", &sequence, ExperienceOutcome::Success);
    assert_eq!(first, second);
    assert!(first.starts_with("exp_"));
}

fn stable_experience_id_changes_when_outcome_changes() {
    let sequence = vec!["grep".to_string(), "file_read".to_string()];
    let success = stable_experience_id("same task", &sequence, ExperienceOutcome::Success);
    let failure = stable_experience_id("same task", &sequence, ExperienceOutcome::Failure);
    assert_ne!(success, failure);
}
