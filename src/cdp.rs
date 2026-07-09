use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{Mutex, RwLock, broadcast, mpsc, oneshot};
use tokio_tungstenite::tungstenite::Message;

// --- Wire format data structures ---

#[derive(Debug, Serialize)]
struct CdpCommand {
    id: u64,
    method: String,
    params: Value,
    #[serde(rename = "sessionId", skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CdpMessage {
    id: Option<u64>,
    method: Option<String>,
    result: Option<Value>,
    error: Option<CdpErrorData>,
    params: Option<Value>,
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CdpErrorData {
    pub code: i64,
    pub message: String,
}

// Event data — used by console/network event buffering
#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    /// The flat-session target this event came from. None for browser-level
    /// events (e.g. Target.targetDestroyed) that aren't scoped to a tab.
    #[allow(dead_code)] // available for future per-tab event filtering
    pub session_id: Option<String>,
}

// --- Connection data ---

/// A single WebSocket connection to the browser-level CDP endpoint. Tabs are
/// never separate connections — each is a flat session attached on top of
/// this one socket (see `attach_session`), so many tabs can be acted on
/// concurrently with no shared "current tab" to fight over.
pub struct CdpConnection {
    cmd_sender: mpsc::UnboundedSender<CdpCommand>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value, CdpErrorData>>>>>,
    event_sender: broadcast::Sender<CdpEvent>,
    next_id: AtomicU64,
    /// Flat CDP sessions attached on this connection, keyed by target_id.
    /// Populated lazily by `attach_session` and reused on every subsequent
    /// call for that target — attaching is a caller claiming its own tab,
    /// never a swap of what any other caller is looking at.
    sessions: Arc<Mutex<HashMap<String, String>>>,
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
    Timeout,
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
            CdpError::Timeout => write!(f, "CDP command timed out"),
        }
    }
}

impl std::error::Error for CdpError {}

// --- Free functions operating on connection data ---

/// Connect to a CDP WebSocket endpoint (the browser-level endpoint — see
/// `browser::browser_ws_url`). One connection serves the whole browser;
/// individual tabs are reached by attaching flat sessions on top of it.
pub async fn connect(ws_url: &str) -> Result<CdpConnection, CdpError> {
    use futures_util::{SinkExt, StreamExt};

    // Edge echoes back ws://localhost:... in webSocketDebuggerUrl. On Windows localhost
    // resolves to ::1 first, where the DevTools server isn't bound — the connect hangs
    // to timeout instead of refusing fast. Pin to the IPv4 literal it actually listens on.
    let ws_url = ws_url.replace("://localhost:", "://127.0.0.1:");

    let (ws_stream, _) = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        tokio_tungstenite::connect_async(&ws_url),
    )
    .await
    .map_err(|_| CdpError::ConnectionFailed("WebSocket connect timed out".to_owned()))?
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
                    session_id: parsed.session_id,
                });
            }
        }

        // WebSocket closed — drop all pending senders so in-flight callers
        // get an immediate ResponseDropped instead of waiting for the 30s timeout.
        // ResponseDropped triggers auto-reconnect in exec_with_reconnect.
        {
            let mut map = pending_clone.lock().await;
            let count = map.len();
            map.drain().for_each(|(_, sender)| { drop(sender); });
            if count > 0 {
                tracing::debug!("WebSocket closed: dropped {count} pending responses");
            }
        }
        drop(writer_handle);
    });

    // Detach the reader — it runs until the WebSocket closes
    drop(reader_handle);

    Ok(CdpConnection {
        cmd_sender,
        pending,
        event_sender,
        next_id: AtomicU64::new(1),
        sessions: Arc::new(Mutex::new(HashMap::new())),
    })
}

/// Send a CDP command and wait for its response. `session_id` scopes the
/// command to a specific tab's flat session (see `attach_session`) — `None`
/// means a browser-level command (Target.*, etc).
pub async fn send(
    conn: &CdpConnection,
    session_id: Option<&str>,
    method: &str,
    params: Value,
) -> Result<Value, CdpError> {
    let id = conn.next_id.fetch_add(1, Ordering::Relaxed);
    let (response_tx, response_rx) = oneshot::channel();

    // Register pending response
    conn.pending.lock().await.insert(id, response_tx);

    // Send command
    let cmd = CdpCommand {
        id,
        method: method.to_owned(),
        params,
        session_id: session_id.map(|s| s.to_owned()),
    };
    conn.cmd_sender
        .send(cmd)
        .map_err(|_| CdpError::SendFailed)?;

    // Wait for response with a 30-second timeout
    match tokio::time::timeout(std::time::Duration::from_secs(30), response_rx).await {
        Ok(Ok(result)) => result.map_err(|e| CdpError::ResponseError {
            code: e.code,
            message: e.message,
        }),
        Ok(Err(_)) => Err(CdpError::ResponseDropped),
        Err(_) => {
            // Timed out — clean up the pending entry to avoid memory leak
            conn.pending.lock().await.remove(&id);
            Err(CdpError::Timeout)
        }
    }
}

/// Subscribe to CDP events (console messages, network requests, etc.).
pub fn subscribe_events(conn: &CdpConnection) -> broadcast::Receiver<CdpEvent> {
    conn.event_sender.subscribe()
}

/// Send a command built by a commands.rs function, scoped to `session_id`.
pub async fn execute(
    conn: &CdpConnection,
    session_id: Option<&str>,
    command: (&str, Value),
) -> Result<Value, CdpError> {
    let (method, params) = command;
    send(conn, session_id, method, params).await
}

/// Send a sequence of commands (e.g., click = mousePressed + mouseReleased),
/// all scoped to the same `session_id`.
pub async fn execute_sequence(
    conn: &CdpConnection,
    session_id: Option<&str>,
    commands: Vec<(&str, Value)>,
) -> Result<Value, CdpError> {
    let mut last_result = Value::Null;
    for (method, params) in commands {
        last_result = send(conn, session_id, method, params).await?;
    }
    Ok(last_result)
}

/// Attach a flat CDP session to a target, returning its sessionId. Idempotent —
/// repeated calls for the same target_id reuse the cached session instead of
/// re-attaching. This is how a caller claims a specific tab: no other caller's
/// session is touched, and no global "current tab" pointer is repointed. Many
/// targets can be attached concurrently on this one connection.
pub async fn attach_session(conn: &CdpConnection, target_id: &str) -> Result<String, CdpError> {
    let mut sessions = conn.sessions.lock().await;
    if let Some(existing) = sessions.get(target_id) {
        return Ok(existing.clone());
    }

    let result = send(
        conn,
        None,
        "Target.attachToTarget",
        serde_json::json!({ "targetId": target_id, "flatten": true }),
    )
    .await?;

    let session_id = result
        .get("sessionId")
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            CdpError::ConnectionFailed("Target.attachToTarget returned no sessionId".to_owned())
        })?
        .to_owned();

    // Enable the domains every tool needs, scoped to this session only.
    execute(conn, Some(&session_id), crate::commands::enable_page()).await?;
    execute(conn, Some(&session_id), crate::commands::enable_dom()).await?;
    execute(conn, Some(&session_id), crate::commands::enable_runtime()).await?;
    execute(conn, Some(&session_id), crate::commands::enable_network()).await?;
    // Stealth: inject script before any page JS to hide CDP signals
    execute(conn, Some(&session_id), crate::commands::add_stealth_script()).await?;

    sessions.insert(target_id.to_owned(), session_id.clone());
    Ok(session_id)
}

/// Drop a cached session (e.g. after closing its tab) so a stale sessionId
/// never gets reused for a target that no longer exists.
pub async fn detach_session(conn: &CdpConnection, target_id: &str) {
    conn.sessions.lock().await.remove(target_id);
}

// --- Swappable connection for reconnect ---

/// Holds the active browser-level CDP connection. All tools read through this.
/// Only replaced when the browser itself dies and is relaunched — tab-to-tab
/// targeting never touches this, it just attaches another session on top.
pub struct LiveConnection {
    inner: RwLock<Option<Arc<CdpConnection>>>,
}

impl std::fmt::Debug for LiveConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LiveConnection").finish()
    }
}

impl LiveConnection {
    /// Create an empty LiveConnection — first tool call will trigger lazy connect.
    pub fn empty() -> Self {
        Self {
            inner: RwLock::new(None),
        }
    }

    /// Get a snapshot of the current connection, or None if not yet connected.
    pub async fn get(&self) -> Option<Arc<CdpConnection>> {
        self.inner.read().await.clone()
    }

    /// Swap to a new connection (browser relaunch/reconnect only).
    /// Drains all pending responses on the old connection so in-flight
    /// callers get an immediate error instead of waiting for the 30s timeout.
    pub async fn swap(&self, new_conn: CdpConnection) {
        let mut guard = self.inner.write().await;
        if let Some(old) = guard.take() {
            let mut pending = old.pending.lock().await;
            let count = pending.len();
            pending.drain().for_each(|(_, sender)| {
                let _ = sender.send(Err(CdpErrorData {
                    code: -1,
                    message: "Connection replaced".to_owned(),
                }));
            });
            if count > 0 {
                tracing::debug!("Drained {count} pending responses from old connection");
            }
        }
        *guard = Some(Arc::new(new_conn));
    }
}
