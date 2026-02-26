use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

// --- Wire format data structures ---

#[derive(Debug, Serialize)]
struct CdpCommand {
    id: u64,
    method: String,
    params: Value,
}

#[derive(Debug, Deserialize)]
struct CdpMessage {
    id: Option<u64>,
    method: Option<String>,
    result: Option<Value>,
    error: Option<CdpErrorData>,
    params: Option<Value>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CdpErrorData {
    pub code: i64,
    pub message: String,
}

// Event data — reserved for WebMCP event listening
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
}

// --- Connection data ---

pub struct CdpConnection {
    cmd_sender: mpsc::UnboundedSender<CdpCommand>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpErrorData>>>>>,
    #[allow(dead_code)]
    event_sender: broadcast::Sender<CdpEvent>,
    next_id: AtomicU64,
}

impl std::fmt::Debug for CdpConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CdpConnection")
            .field("next_id", &self.next_id.load(Ordering::Relaxed))
            .finish()
    }
}

#[derive(Debug)]
pub enum CdpError {
    ConnectionFailed(String),
    SendFailed,
    ResponseError { code: i64, message: String },
    ResponseDropped,
}

impl std::fmt::Display for CdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CdpError::ConnectionFailed(msg) => write!(f, "CDP connection failed: {msg}"),
            CdpError::SendFailed => write!(f, "Failed to send CDP command"),
            CdpError::ResponseError { code, message } => {
                write!(f, "CDP error ({code}): {message}")
            }
            CdpError::ResponseDropped => write!(f, "CDP response channel dropped"),
        }
    }
}

impl std::error::Error for CdpError {}

// --- Free functions operating on connection data ---

pub async fn connect(ws_url: &str) -> Result<CdpConnection, CdpError> {
    use futures_util::{SinkExt, StreamExt};

    let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url)
        .await
        .map_err(|e| CdpError::ConnectionFailed(e.to_string()))?;

    let (mut ws_write, mut ws_read) = ws_stream.split();
    let (cmd_sender, mut cmd_receiver) = mpsc::unbounded_channel::<CdpCommand>();
    let (event_sender, _) = broadcast::channel::<CdpEvent>(256);
    let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpErrorData>>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // Writer task: takes commands from channel, serializes to WebSocket
    let writer_handle = tokio::spawn(async move {
        while let Some(cmd) = cmd_receiver.recv().await {
            let json = serde_json::to_string(&cmd).unwrap();
            if ws_write.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }
    });

    // Reader task: reads WebSocket, routes responses and events
    let pending_clone = pending.clone();
    let event_sender_clone = event_sender.clone();
    let reader_handle = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_read.next().await {
            let text = match msg {
                Message::Text(t) => t,
                _ => continue,
            };

            let parsed: CdpMessage = match serde_json::from_str(&text) {
                Ok(m) => m,
                Err(_) => continue,
            };

            // Response (has id) → route to pending sender
            if let Some(id) = parsed.id {
                let mut map = pending_clone.lock().await;
                if let Some(sender) = map.remove(&id) {
                    if let Some(err) = parsed.error {
                        let _ = sender.send(Err(err));
                    } else {
                        let _ = sender.send(Ok(parsed.result.unwrap_or(Value::Null)));
                    }
                }
            }
            // Event (has method, no id) → broadcast
            else if let Some(method) = parsed.method {
                let _ = event_sender_clone.send(CdpEvent {
                    method,
                    params: parsed.params.unwrap_or(Value::Null),
                });
            }
        }

        // WebSocket closed — drop all pending senders
        drop(writer_handle);
    });

    // Detach the reader — it runs until the WebSocket closes
    drop(reader_handle);

    Ok(CdpConnection {
        cmd_sender,
        pending,
        event_sender,
        next_id: AtomicU64::new(1),
    })
}

/// Send a CDP command and wait for its response.
pub async fn send(conn: &CdpConnection, method: &str, params: Value) -> Result<Value, CdpError> {
    let id = conn.next_id.fetch_add(1, Ordering::Relaxed);
    let (response_tx, response_rx) = oneshot::channel();

    // Register pending response
    conn.pending.lock().await.insert(id, response_tx);

    // Send command
    let cmd = CdpCommand {
        id,
        method: method.to_owned(),
        params,
    };
    conn.cmd_sender
        .send(cmd)
        .map_err(|_| CdpError::SendFailed)?;

    // Wait for response
    let result = response_rx.await.map_err(|_| CdpError::ResponseDropped)?;
    result.map_err(|e| CdpError::ResponseError {
        code: e.code,
        message: e.message,
    })
}

/// Subscribe to CDP events — reserved for WebMCP event listening.
#[allow(dead_code)]
pub fn subscribe_events(conn: &CdpConnection) -> broadcast::Receiver<CdpEvent> {
    conn.event_sender.subscribe()
}

/// Send a command built by a commands.rs function.
pub async fn execute(conn: &CdpConnection, command: (&str, Value)) -> Result<Value, CdpError> {
    let (method, params) = command;
    send(conn, method, params).await
}

/// Send a sequence of commands (e.g., click = mousePressed + mouseReleased).
pub async fn execute_sequence(
    conn: &CdpConnection,
    commands: Vec<(&str, Value)>,
) -> Result<Value, CdpError> {
    let mut last_result = Value::Null;
    for (method, params) in commands {
        last_result = send(conn, method, params).await?;
    }
    Ok(last_result)
}
