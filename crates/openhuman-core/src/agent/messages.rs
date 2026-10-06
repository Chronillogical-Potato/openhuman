//! Wire adapters for the message rows OpenHuman persists itself.
//!
//! The runtime, the transcript writer and the host all speak the one
//! `tinyagents_session::transcript::TranscriptMessage` row; there is no
//! host-owned message type. What remains here is the compatibility contract for
//! the two host-owned files that embed rows:
//!
//! - the durable sub-agent session store (`subagent_sessions.json`,
//!   `latestHistory`), and
//! - the sub-agent pause checkpoint (`history`).
//!
//! Both were written as `{ "role", "content" }` pairs only: the old host type
//! never serialized its `id`, `extra_metadata` or `cache_breakpoints`.
//! [`history_wire`] keeps writing exactly that shape, so a binary of either
//! generation reads what the other wrote, and reads any row shape (the
//! vendor row is a superset whose extra fields default).
//!
//! The in-memory row is typed (tool calls, call ids and image parts are
//! fields, `content` is plain text), but these files stay the flat string
//! pairs they always were: a write rebuilds the established string form
//! (`legacy_content`: the native envelope for a tool round, `[IMAGE:<url>]`
//! markers for image parts) and a read lifts it back into the typed row.

use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tinyagents_session::transcript::TranscriptMessage;

#[derive(Serialize)]
struct WireRow<'a> {
    role: &'a str,
    content: &'a str,
}

/// The flat `content` string a typed row is stored as. Image parts use the
/// host's own `[IMAGE:<url>]` marker (which both generations read), not the
/// private native-wire marker.
fn wire_content(row: &TranscriptMessage) -> String {
    if row.parts.is_some() {
        row.display_content()
    } else {
        row.legacy_content()
    }
}

/// A deserialized row with any string-encoded structure lifted into the typed
/// fields.
fn lifted(row: TranscriptMessage) -> TranscriptMessage {
    row.normalized()
}

fn serialize_rows<S: Serializer>(
    rows: &[TranscriptMessage],
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let mut seq = serializer.serialize_seq(Some(rows.len()))?;
    for row in rows {
        seq.serialize_element(&WireRow {
            role: &row.role,
            content: &wire_content(row),
        })?;
    }
    seq.end()
}

/// `#[serde(with = "…::history_wire")]` for a `Vec<TranscriptMessage>` field.
pub mod history_wire {
    use super::*;

    pub fn serialize<S: Serializer>(
        rows: &[TranscriptMessage],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serialize_rows(rows, serializer)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<TranscriptMessage>, D::Error> {
        Ok(Vec::<TranscriptMessage>::deserialize(deserializer)?
            .into_iter()
            .map(lifted)
            .collect())
    }

    /// The same contract for an `Option<Vec<TranscriptMessage>>` field.
    pub mod option {
        use super::*;

        pub fn serialize<S: Serializer>(
            rows: &Option<Vec<TranscriptMessage>>,
            serializer: S,
        ) -> Result<S::Ok, S::Error> {
            match rows {
                Some(rows) => serialize_rows(rows, serializer),
                None => serializer.serialize_none(),
            }
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Option<Vec<TranscriptMessage>>, D::Error> {
            Ok(Option::<Vec<TranscriptMessage>>::deserialize(deserializer)?
                .map(|rows| rows.into_iter().map(lifted).collect()))
        }
    }
}

#[cfg(test)]
#[path = "messages_tests.rs"]
mod tests;
