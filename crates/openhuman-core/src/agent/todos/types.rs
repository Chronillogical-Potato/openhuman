//! TinyAgents todo types and OpenHuman wire-format normalization.

use chrono::{TimeZone, Utc};

pub use tinyagents_graph::todos::{TaskApprovalMode, TaskBoard, TaskBoardCard, TaskCardStatus};

pub(crate) fn normalize_timestamp_for_wire(value: &str) -> String {
    if chrono::DateTime::parse_from_rfc3339(value).is_ok() {
        return value.to_owned();
    }
    if let Ok(updated_at_ms) = value.parse::<i64>() {
        if let Some(updated_at) = Utc.timestamp_millis_opt(updated_at_ms).single() {
            return updated_at.to_rfc3339();
        }
    }
    tracing::warn!(updated_at = %value, "invalid todo timestamp; using current time");
    Utc::now().to_rfc3339()
}

pub(crate) fn normalize_cards_for_wire(cards: &mut [TaskBoardCard]) {
    for card in cards {
        card.updated_at = normalize_timestamp_for_wire(&card.updated_at);
    }
}
