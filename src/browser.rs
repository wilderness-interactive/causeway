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

    // If using the default profile (dedicated_profile = false), we need to kill any
    // existing browser first. Chromium ignores --remote-debugging-port when attaching
    // to an already-running instance using the same profile.
    if !config.dedicated_profile {
        let exe_name = extract_exe_name(&config.executable);
        if is_process_running(&exe_name) {
            tracing::info!("Killing existing {exe_name} to relaunch with CDP");
            kill_process(&exe_name);
            // Give the OS a moment to release the profile lock
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }

    let mut args = vec![
        format!("--remote-debugging-port={}", config.port),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
    ];

    // Only use a separate profile dir when explicitly requested
    if config.dedicated_profile {
        let causeway_profile = std::env::temp_dir().join("causeway-profile");
        tracing::info!("Profile: {}", causeway_profile.display());
        args.push(format!("--user-data-dir={}", causeway_profile.display()));
    }

    // Restore last session so tabs persist across restarts
    if config.restore_session {
        args.push("--restore-last-session".to_owned());
    }

    // Load unpacked extensions
    if !config.extensions.is_empty() {
        let paths = config.extensions.join(",");
        args.push(format!("--load-extension={paths}"));
        tracing::info!("Loading extensions: {paths}");
    }

    tracing::info!("Launching browser: {}", config.executable);
    let child = Command::new(&config.executable)
        .args(&args)
        .spawn()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

    let ws_url = poll_until_ready(config.port).await?;
    Ok(LaunchResult::Spawned { child, ws_url })
}

/// Extract just the executable filename from a full path (e.g. "brave.exe" from the full path)
fn extract_exe_name(executable: &str) -> String {
    std::path::Path::new(executable)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("brave.exe")
        .to_owned()
}

/// Check if a process with this name is currently running (Windows)
fn is_process_running(exe_name: &str) -> bool {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {exe_name}"), "/NH"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(exe_name)
        }
        Err(_) => false,
    }
}

/// Kill all processes with this name (Windows)
fn kill_process(exe_name: &str) {
    let _ = Command::new("taskkill")
        .args(["/F", "/IM", exe_name])
        .output();
}

/// Find the WebSocket URL for a specific target ID, or the first page target if None.
pub async fn find_target_ws_url(port: u16, target_id: Option<&str>) -> Result<String, BrowserError> {
    let url = format!("http://localhost:{port}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

    let targets: Vec<serde_json::Value> = client
        .get(&url)
        .send()
        .await
        .map_err(|_| BrowserError::Timeout)?
        .json()
        .await
        .map_err(|_| BrowserError::Timeout)?;

    for target in &targets {
        if target.get("type").and_then(|t| t.as_str()) != Some("page") {
            continue;
        }

        if let Some(wanted_id) = target_id {
            let id = target.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if id != wanted_id {
                continue;
            }
        }

        if let Some(ws_url) = target.get("webSocketDebuggerUrl").and_then(|u| u.as_str()) {
            return Ok(ws_url.to_owned());
        }
    }

    Err(BrowserError::Timeout)
}

async fn try_connect_existing(port: u16) -> Result<String, ()> {
    find_target_ws_url(port, None).await.map_err(|_| ())
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
