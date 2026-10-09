//! Who sent a channel message, as the memory actor the user turn is
//! observed from.
//!
//! A message that arrived on a channel (WhatsApp, Telegram, SMS, …) was
//! written by its sender, not by the memory's owner. The channel runtime puts
//! the sender on the turn's origin ([`AgentTurnOrigin::ExternalChannel`]);
//! [`channel_actor`] turns it into an [`ObservedActor`]:
//!
//! | Sender | Actor id |
//! | --- | --- |
//! | a phone number, on a phone-addressed channel | `user:+15551234567` (dial digits) |
//! | an email address | `user:priya@acme.com` (lower-cased) |
//! | anything else (a Telegram or Discord id, a WhatsApp `<lid>@lid`, a Signal UUID) | `<channel>:<sender>` |
//!
//! with the sender's display name as the channel gives it (a push or profile
//! name; no channel exposes a saved contact name). Phone numbers and email
//! addresses are kept in plain text. In a group chat the sender is the
//! participant who wrote the message, not the group.
//!
//! Only a channel that addresses people by phone reads a digit string as a
//! phone number: a Telegram user id or a Discord snowflake is digits too.

use tinymemory_api::ObservedActor;

use crate::agent::turn_origin::AgentTurnOrigin;

/// Channels whose senders are phone numbers (or, for iMessage, email
/// addresses).
const PHONE_CHANNELS: &[&str] = &["whatsapp", "signal", "imessage", "linq", "sms"];

/// The actor a channel turn is observed from, or `None` for any other
/// origin, a channel turn without a sender, or a cron run (which replays its
/// creator's channel origin around the job's own prompt, `cron/origin.rs`).
#[must_use]
pub fn channel_actor(origin: &AgentTurnOrigin) -> Option<ObservedActor> {
    let AgentTurnOrigin::ExternalChannel {
        channel,
        sender: Some(sender),
        sender_name,
        message_id,
        ..
    } = origin
    else {
        return None;
    };
    if message_id.starts_with("cron:") {
        return None;
    }
    Some(ObservedActor {
        id: actor_id(channel, sender)?,
        name: sender_name
            .as_deref()
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_string),
    })
}

/// `user:<phone or email>` or `<channel>:<sender>`; `None` when either is
/// blank or the sender is not one token.
fn actor_id(channel: &str, sender: &str) -> Option<String> {
    let channel = channel.trim().to_ascii_lowercase();
    let sender = sender.trim();
    if channel.is_empty() || channel.contains(char::is_whitespace) || sender.is_empty() {
        return None;
    }
    if PHONE_CHANNELS.contains(&channel.as_str()) && looks_like_phone(sender) {
        return Some(format!("user:{}", dial_digits(sender)));
    }
    if sender.contains(char::is_whitespace) {
        return None;
    }
    if is_email(sender) {
        return Some(format!("user:{}", sender.to_ascii_lowercase()));
    }
    Some(format!("{channel}:{sender}"))
}

/// Only phone characters, with seven or more digits.
fn looks_like_phone(text: &str) -> bool {
    text.chars()
        .all(|c| c.is_ascii_digit() || "+-(). ".contains(c))
        && text.chars().filter(char::is_ascii_digit).count() >= 7
}

/// `text`'s digits, with its leading `+` when it has one.
fn dial_digits(text: &str) -> String {
    let digits: String = text.chars().filter(char::is_ascii_digit).collect();
    if text.starts_with('+') {
        format!("+{digits}")
    } else {
        digits
    }
}

/// One `@`, a non-empty local part and a dotted domain: a WhatsApp
/// `<lid>@lid` is not an email address.
fn is_email(text: &str) -> bool {
    text.split_once('@').is_some_and(|(local, domain)| {
        !local.is_empty() && domain.contains('.') && !domain.contains('@')
    })
}

#[cfg(test)]
#[path = "sender_tests.rs"]
mod tests;
