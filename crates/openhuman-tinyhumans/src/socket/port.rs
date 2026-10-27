use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::RwLock;
use serde_json::Value;
use tinyhumans_sdk::{socket::SocketConnection, TinyHumansClient};
use tokio::sync::{mpsc, oneshot, Mutex};

use openhuman_embed::__host::platform::socket::{
    ConnectionStatus, SocketPort, SocketState, TokenProvider,
};

use super::sdk_token_provider;

enum SocketCommand {
    Emit {
        event: String,
        data: Value,
        result: oneshot::Sender<Result<(), String>>,
    },
    EmitWithAck {
        event: String,
        data: Value,
        timeout: Duration,
        result: oneshot::Sender<Result<Value, String>>,
    },
    Disconnect(oneshot::Sender<()>),
}

/// TinyHumans' SDK-backed implementation of the core socket port.
pub(crate) struct SdkSocketPort {
    state: Arc<RwLock<SocketState>>,
    identity: RwLock<Option<(String, String)>>,
    loop_active: Arc<AtomicBool>,
    stopped_on_failure: Arc<AtomicBool>,
    commands: Mutex<Option<mpsc::Sender<SocketCommand>>>,
    webhook_router: RwLock<Option<Arc<openhuman_embed::__host::skills::webhooks::WebhookRouter>>>,
}

impl Default for SdkSocketPort {
    fn default() -> Self {
        Self {
            state: Arc::new(RwLock::new(SocketState::default())),
            identity: RwLock::new(None),
            loop_active: Arc::new(AtomicBool::new(false)),
            stopped_on_failure: Arc::new(AtomicBool::new(false)),
            commands: Mutex::new(None),
            webhook_router: RwLock::new(None),
        }
    }
}

#[async_trait]
impl SocketPort for SdkSocketPort {
    async fn connect(&self, url: &str, token: &str) -> Result<(), String> {
        let token = token.to_owned();
        self.connect_with_provider(
            url,
            Arc::new(move || {
                let token = token.clone();
                Box::pin(async move {
                    if token.trim().is_empty() {
                        Err("empty session token — authenticate first".to_owned())
                    } else {
                        Ok(token)
                    }
                })
            }),
        )
        .await
    }

    async fn connect_with_provider(&self, url: &str, provider: TokenProvider) -> Result<(), String> {
        let identity_token = provider().await?;
        if identity_token.trim().is_empty() {
            return Err("empty session token — authenticate first".to_owned());
        }
        let handshake_url = crate::backend::url::websocket_url(url);
        if !crate::backend::url::is_safe_socket_endpoint(&handshake_url) {
            return Err("backend socket requires TLS or a loopback endpoint".to_owned());
        }
        self.disconnect().await?;

        let connection = TinyHumansClient::new(url)
            .connect_socket_with_token_provider(sdk_token_provider(provider))
            .await
            .map_err(|error| error.to_string())?;
        let (tx, rx) = mpsc::channel(128);
        *self.commands.lock().await = Some(tx);
        *self.identity.write() = Some((url.to_owned(), identity_token));
        *self.state.write() = SocketState {
            status: ConnectionStatus::Connecting,
            socket_id: None,
            error: None,
        };
        self.loop_active.store(true, Ordering::Release);
        self.stopped_on_failure.store(false, Ordering::Release);
        tokio::spawn(run_socket_actor(
            connection,
            rx,
            Arc::clone(&self.state),
            Arc::clone(&self.loop_active),
            Arc::clone(&self.stopped_on_failure),
        ));
        Ok(())
    }

    async fn disconnect(&self) -> Result<(), String> {
        if let Some(commands) = self.commands.lock().await.take() {
            let (sent, received) = oneshot::channel();
            if commands.send(SocketCommand::Disconnect(sent)).await.is_ok() {
                let _ = received.await;
            }
        }
        self.loop_active.store(false, Ordering::Release);
        self.stopped_on_failure.store(false, Ordering::Release);
        *self.identity.write() = None;
        *self.state.write() = SocketState::default();
        Ok(())
    }

    async fn emit(&self, event: &str, data: Value) -> Result<(), String> {
        if !self.is_connected() {
            return Err("Not connected".to_owned());
        }
        let commands = self.commands.lock().await.clone().ok_or("Not connected")?;
        let (result, response) = oneshot::channel();
        commands
            .send(SocketCommand::Emit {
                event: event.to_owned(),
                data,
                result,
            })
            .await
            .map_err(|_| "Socket not connected".to_owned())?;
        response.await.map_err(|_| "Socket not connected".to_owned())?
    }

    async fn emit_with_ack(&self, event: &str, data: Value, timeout: Duration) -> Result<Value, String> {
        let commands = self.commands.lock().await.clone().ok_or("Not connected")?;
        let (result, response) = oneshot::channel();
        commands
            .send(SocketCommand::EmitWithAck {
                event: event.to_owned(),
                data,
                timeout,
                result,
            })
            .await
            .map_err(|_| "Socket not connected".to_owned())?;
        response.await.map_err(|_| "Socket not connected".to_owned())?
    }

    fn get_state(&self) -> SocketState {
        self.state.read().clone()
    }

    fn is_connected(&self) -> bool {
        self.state.read().status == ConnectionStatus::Connected
    }

    fn is_live_for(&self, url: &str, token: &str) -> bool {
        let state = self.state.read();
        (matches!(state.status, ConnectionStatus::Connected)
            || (state.status == ConnectionStatus::Connecting
                && self.loop_active.load(Ordering::Acquire)))
            && self.identity.read().as_ref().is_some_and(|(active_url, active_token)| {
                active_url == url && active_token == token
            })
    }

    fn status_for(&self, url: &str, token: &str) -> Option<ConnectionStatus> {
        self.identity
            .read()
            .as_ref()
            .filter(|(active_url, active_token)| active_url == url && active_token == token)
            .map(|_| self.state.read().status)
    }

    fn is_loop_active(&self) -> bool {
        self.loop_active.load(Ordering::Acquire)
    }

    fn loop_stopped_on_failure(&self) -> bool {
        self.stopped_on_failure.load(Ordering::Acquire)
    }

    fn set_webhook_router(&self, router: Arc<openhuman_embed::__host::skills::webhooks::WebhookRouter>) {
        *self.webhook_router.write() = Some(router);
    }

    fn webhook_router(&self) -> Option<Arc<openhuman_embed::__host::skills::webhooks::WebhookRouter>> {
        self.webhook_router.read().clone()
    }
}

async fn run_socket_actor(
    mut connection: SocketConnection,
    mut commands: mpsc::Receiver<SocketCommand>,
    state: Arc<RwLock<SocketState>>,
    loop_active: Arc<AtomicBool>,
    stopped_on_failure: Arc<AtomicBool>,
) {
    loop {
        tokio::select! {
            event = connection.next_event() => {
                let Some(event) = event else {
                    loop_active.store(false, Ordering::Release);
                    stopped_on_failure.store(connection.stopped_on_failure(), Ordering::Release);
                    if connection.stopped_on_failure() {
                        state.write().status = ConnectionStatus::Error;
                    }
                    break;
                };
                match event.name.as_str() {
                    "open" => state.write().status = ConnectionStatus::Connecting,
                    "connect" => state.write().status = ConnectionStatus::Connected,
                    "close" => {
                        state.write().status = ConnectionStatus::Reconnecting;
                        loop_active.store(true, Ordering::Release);
                    }
                    "error" => {
                        let mut snapshot = state.write();
                        snapshot.status = ConnectionStatus::Reconnecting;
                        snapshot.error = event.payload.decode::<Value>("error").ok().and_then(|value| value.as_str().map(str::to_owned));
                        stopped_on_failure.store(connection.stopped_on_failure(), Ordering::Release);
                    }
                    _ => {
                        let data = event.payload.decode::<Value>(&event.name).unwrap_or(Value::Null);
                        openhuman_embed::__host::platform::socket::dispatch_socket_event(&event.name, data);
                    }
                }
            }
            command = commands.recv() => {
                let Some(command) = command else { break; };
                match command {
                    SocketCommand::Emit { event, data, result } => {
                        let _ = result.send(connection.emit(&event, &data).await.map_err(|error| error.to_string()));
                    }
                    SocketCommand::EmitWithAck { event, data, timeout, result } => {
                        let value = connection
                            .emit_with_ack::<Value, Value>(&event, &data, timeout)
                            .await
                            .map_err(|error| error.to_string());
                        let _ = result.send(value);
                    }
                    SocketCommand::Disconnect(result) => {
                        let _ = connection.disconnect().await;
                        loop_active.store(false, Ordering::Release);
                        let _ = result.send(());
                        break;
                    }
                }
            }
        }
    }
}
