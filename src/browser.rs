use std::process::{Child, Command};
use std::time::Duration;

use crate::config::BrowserConfig;

/// Launch result: either we spawned a new browser, or connected to an existing one.
pub enum LaunchResult {
    Spawned { child: Child, ws_url: String },
    Existing { ws_url: String },
}

pub async fn launch(config: &BrowserConfig) -> Result<LaunchResult, BrowserError> {
    // Check if CDP is already available (browser already running with debugging port)
    if let Ok(ws_url) = try_connect_existing(config.port).await {
        tracing::info!("Found existing browser with CDP on port {}", config.port);
        return Ok(LaunchResult::Existing { ws_url });
    }

    // Chromium ignores --remote-debugging-port when attaching to an already-running
    // instance using the same profile. We must use a separate user-data-dir to guarantee
    // a fresh instance that actually enables CDP.
    //
    // The Causeway profile dir persists across runs so bookmarks/logins survive,
    // but it's separate from your daily browser profile.
    let causeway_profile = std::env::temp_dir().join("causeway-profile");

    let mut args = vec![
        format!("--remote-debugging-port={}", config.port),
        format!("--user-data-dir={}", causeway_profile.display()),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
    ];

    // Restore last session so tabs persist across Causeway restarts
    if config.restore_session {
        args.push("--restore-last-session".to_owned());
    }

    tracing::info!("Launching browser: {}", config.executable);
    tracing::info!("Profile: {}", causeway_profile.display());
    let child = Command::new(&config.executable)
        .args(&args)
        .spawn()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

    let ws_url = poll_until_ready(config.port).await?;
    Ok(LaunchResult::Spawned { child, ws_url })
}

async fn try_connect_existing(port: u16) -> Result<String, ()> {
    let url = format!("http://localhost:{port}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .map_err(|_| ())?;

    let targets: Vec<serde_json::Value> = client
        .get(&url)
        .send()
        .await
        .map_err(|_| ())?
        .json()
        .await
        .map_err(|_| ())?;

    for target in &targets {
        if target.get("type").and_then(|t| t.as_str()) == Some("page") {
            if let Some(ws_url) = target.get("webSocketDebuggerUrl").and_then(|u| u.as_str()) {
                return Ok(ws_url.to_owned());
            }
        }
    }

    Err(())
}

async fn poll_until_ready(port: u16) -> Result<String, BrowserError> {
    let url = format!("http://localhost:{port}/json");
    let client = reqwest::Client::new();

    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => continue,
        };

        let targets: Vec<serde_json::Value> = match response.json().await {
            Ok(t) => t,
            Err(_) => continue,
        };

        // Find the first "page" target with a WebSocket URL
        for target in &targets {
            if target.get("type").and_then(|t| t.as_str()) == Some("page") {
                if let Some(ws_url) = target.get("webSocketDebuggerUrl").and_then(|u| u.as_str()) {
                    tracing::info!("CDP ready: {ws_url}");
                    return Ok(ws_url.to_owned());
                }
            }
        }
    }

    Err(BrowserError::Timeout)
}

pub fn shutdown(child: Option<Child>) {
    if let Some(mut c) = child {
        let _ = c.kill();
        let _ = c.wait();
        tracing::info!("Browser process terminated");
    } else {
        tracing::info!("Browser was pre-existing, not terminating");
    }
}

#[derive(Debug)]
pub enum BrowserError {
    LaunchFailed(String),
    Timeout,
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BrowserError::LaunchFailed(msg) => write!(f, "Failed to launch browser: {msg}"),
            BrowserError::Timeout => write!(f, "Browser did not become ready within 15 seconds"),
        }
    }
}

impl std::error::Error for BrowserError {}
