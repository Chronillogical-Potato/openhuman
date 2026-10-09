//! `GET /ws/live-voice`: the authenticated live voice agent WebSocket.
//!
//! Same upgrade guard as `/ws/dictation` (origin allowlist plus the core bearer
//! from the `Authorization` header or `?token=`); the session itself is
//! `openhuman_core::voice::live::ws::handle_live_voice_ws`.

use std::sync::Arc;

use axum::extract::{Query, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};

use super::dictation::{authorize_dictation_request, DictationQuery};

/// WebSocket upgrade handler for live voice sessions.
pub(super) async fn live_voice_ws_handler(
    headers: axum::http::HeaderMap,
    Query(query): Query<DictationQuery>,
    ws: WebSocketUpgrade,
) -> Response {
    log::info!("[ws] live voice WebSocket upgrade requested");
    if let Err(error) = authorize_dictation_request(&headers, &query) {
        return error.into_response();
    }
    // Agent audio arrives in ~40 ms chunks; keep frames small and unbounded
    // in count rather than buffering.
    ws.on_upgrade(|socket| async move {
        let config = match openhuman_core::config::rpc::load_config_with_timeout().await {
            Ok(c) => Arc::new(c),
            Err(e) => {
                log::error!("[ws] failed to load config for live voice: {e}");
                return;
            }
        };
        openhuman_core::voice::live::ws::handle_live_voice_ws(socket, config).await;
    })
}
