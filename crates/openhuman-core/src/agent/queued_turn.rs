//! Host-owned payload for a message queued while an agent turn is active.
//!
//! TinyAgents owns the queue mechanics and its [`QueueLane`](tinyagents_harness::run_queue::QueueLane)
//! selects how this payload is consumed. OpenHuman retains the web and
//! orchestration metadata needed to dispatch a deferred follow-up turn.

/// A queued OpenHuman turn input.
///
/// The payload deliberately does not carry a lane: lane selection is made at
/// the host boundary when the item is pushed into TinyAgents' `RunQueue`.
#[derive(Debug, Clone)]
pub struct QueuedTurn {
    /// Stable id for this queued item (minted once, at push time). Carried on
    /// `RunQueue*` domain events (`item_id`) and the `queue_item_*` web-channel
    /// events so the frontend can key a queued-message row and later target it
    /// with `channel.web_queue_remove`.
    pub id: String,
    pub text: String,
    pub client_id: String,
    pub thread_id: String,
    pub queued_at_ms: u64,
    pub model_override: Option<String>,
    pub temperature: Option<f64>,
    pub locale: Option<String>,
}

/// Preview a queued message's caption or attachment names, without exposing
/// inline upload payloads. This also matches previews made before staging by
/// the web composer, so cancellation finds the same pending follow-up.
#[must_use]
pub fn text_preview(text: &str) -> String {
    use std::sync::LazyLock;

    static MARKERS: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"\[(IMAGE|FILE|ATTACHMENT):([^\]]+)\]")
            .expect("queue attachment pattern is valid")
    });
    static SPACES: LazyLock<regex::Regex> =
        LazyLock::new(|| regex::Regex::new(" {2,}").expect("space pattern is valid"));

    let mut names = Vec::new();
    let caption = MARKERS.replace_all(text, |capture: &regex::Captures<'_>| {
        if &capture[1] == "ATTACHMENT" {
            let (_, files) = super::attachments::parse(&capture[0]);
            let Some(file) = files.into_iter().next() else {
                return capture[0].to_owned();
            };
            names.push(file.name);
        } else {
            // Decode only the header's name parameter; the payload stays opaque.
            let header = capture[2]
                .strip_prefix("data:")
                .map(|source| source.split(',').next().unwrap_or_default())
                .unwrap_or_default();
            let name = header
                .split(';')
                .find_map(|parameter| parameter.strip_prefix("name="))
                .and_then(|encoded| {
                    let parameter = format!("name={encoded}");
                    url::form_urlencoded::parse(parameter.as_bytes())
                        .next()
                        .map(|(_, value)| value.into_owned())
                })
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| "attachment".into());
            names.push(name);
        }
        String::new()
    });
    let caption = SPACES.replace_all(&caption, " ");
    let caption = caption.trim();
    let preview = if caption.is_empty() {
        names.join(", ")
    } else {
        caption.to_owned()
    };
    crate::core::events::clip_to_chars(&preview, 80)
}

#[cfg(test)]
#[path = "queued_turn_tests.rs"]
mod tests;
