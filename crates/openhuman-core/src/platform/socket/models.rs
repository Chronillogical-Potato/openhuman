use serde::{Deserialize, Serialize};

/// Socket connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum ConnectionStatus {
    #[default]
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Error,
}

/// Socket connection state emitted to frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SocketState {
    pub status: ConnectionStatus,
    pub socket_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl Default for SocketState {
    fn default() -> Self {
        Self {
            status: ConnectionStatus::Disconnected,
            socket_id: None,
            error: None,
        }
    }
}
