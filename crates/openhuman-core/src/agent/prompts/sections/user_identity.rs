//! Rendering of the authenticated user's non-secret identity prompt block.

use super::super::types::{PromptContext, PromptSection, PromptTier};
use anyhow::Result;
use std::fmt::Write;

/// Renders the authenticated user's non-secret identity fields
/// (`id` / `name` / `email`) into the system prompt — see issue #926.
///
/// Empty when [`PromptContext::user_identity`] is `None` or the
/// identity has no populated fields. Tokens, refresh tokens, and any
/// opaque credential material are forbidden — only the three
/// identifying fields ship.
pub struct UserIdentitySection;

impl PromptSection for UserIdentitySection {
    fn tier(&self) -> PromptTier {
        // The signed-in user, which changes on login and on logout.
        PromptTier::Volatile
    }

    fn name(&self) -> &str {
        "user_identity"
    }

    fn build(&self, ctx: &PromptContext<'_>) -> Result<String> {
        let identity = match ctx.user_identity.as_ref() {
            Some(id) if !id.is_empty() => id,
            _ => return Ok(String::new()),
        };

        let mut fields = String::new();
        if let Some(name) = identity.name.as_deref().filter(|s| !s.trim().is_empty()) {
            let _ = writeln!(fields, "- name: {}", sanitize_identity_field(name));
        }
        if let Some(email) = identity.email.as_deref().filter(|s| !s.trim().is_empty()) {
            let _ = writeln!(fields, "- email: {}", sanitize_identity_field(email));
        }
        if let Some(id) = identity.id.as_deref().filter(|s| !s.trim().is_empty()) {
            let _ = writeln!(fields, "- id: {}", sanitize_identity_field(id));
        }
        if fields.trim().is_empty() {
            return Ok(String::new());
        }

        let mut out = String::from("## User\n\n");
        out.push_str(
            "The signed-in user is identified below. Use these fields directly in tool \
             calls and do not ask the user to repeat them.\n\n",
        );
        out.push_str(&fields);
        Ok(out.trim_end().to_string())
    }
}

/// Collapse whitespace in a user-identity field for a single markdown bullet.
fn sanitize_identity_field(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
