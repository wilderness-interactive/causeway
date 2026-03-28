use std::process::Command;
use std::time::Duration;

use crate::config::BrowserConfig;

/// Launch result: either we spawned a new browser, or connected to an existing one.
pub enum LaunchResult {
    Spawned { ws_url: String },
    Existing { ws_url: String },
}

pub async fn launch(config: &BrowserConfig) -> Result<LaunchResult, BrowserError> {
    // Check if CDP is already available (browser already running with debugging port)
    if let Ok(ws_url) = try_connect_existing(config.port).await {
        tracing::info!("Found existing browser with CDP on port {}", config.port);
        return Ok(LaunchResult::Existing { ws_url });
    }

    // If we got here, CDP isn't available on the port. Chromium ignores
    // --remote-debugging-port when piggybacking on an existing process (even
    // background processes with no visible window). Kill them so the fresh
    // spawn gets the flag. Safe for other Causeway instances: if any had CDP
    // active, try_connect_existing above would have already connected.
    let exe_name = extract_exe_name(&config.executable);
    if is_process_running(&exe_name) {
        tracing::info!("Killing existing {exe_name} — CDP unavailable, must relaunch with debugging port");
        kill_and_wait(&exe_name).await?;
    }

    let mut args = vec![
        format!("--remote-debugging-port={}", config.port),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
    ];

    // Dedicated profile: separate user-data-dir lets Chromium launch as an independent
    // process even if another instance of the same browser is already running.
    if config.dedicated_profile {
        let data_dir = config.user_data_dir.clone().unwrap_or_else(|| {
            std::env::temp_dir().join("causeway-profile").to_string_lossy().into_owned()
        });
        tracing::info!("User data dir: {data_dir}");
        args.push(format!("--user-data-dir={data_dir}"));
        if let Some(ref profile_name) = config.profile {
            tracing::info!("Profile: {profile_name}");
            args.push(format!("--profile-directory={profile_name}"));
        }
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
    Command::new(&config.executable)
        .args(&args)
        .spawn()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

    // Poll until CDP is available and targets have stabilized (no more session restore churn)
    let ws_url = poll_until_stable(config.port).await?;
    Ok(LaunchResult::Spawned { ws_url })
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

/// Kill all processes with this name and wait until they're actually gone.
/// Retries the kill if processes survive, because Chromium spawns many child
/// processes that can respawn or linger (crashpad, updater, GPU process).
async fn kill_and_wait(exe_name: &str) -> Result<(), BrowserError> {
    // Kill, check, re-kill if needed. 30s total outer bound.
    for tick in 0..120 {
        if !is_process_running(exe_name) {
            tracing::info!("{exe_name} fully terminated");
            return Ok(());
        }

        // Kill on first tick and every 3 seconds thereafter
        if tick % 12 == 0 {
            let attempt = tick / 12 + 1;
            tracing::info!("taskkill attempt {attempt} for {exe_name}");
            match Command::new("taskkill").args(["/F", "/IM", exe_name]).output() {
                Ok(o) if !o.status.success() => {
                    tracing::warn!("taskkill: {}", String::from_utf8_lossy(&o.stderr).trim());
                }
                Err(e) => tracing::warn!("taskkill error: {e}"),
                Ok(_) => {}
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(BrowserError::LaunchFailed(
        format!("Could not kill {exe_name} after 30s — is another program holding it?")
    ))
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

/// Poll until CDP is available AND page targets have stabilized.
/// Returns the WS URL of the first stable page target.
/// Handles both slow browser launches and session restore target churn.
async fn poll_until_stable(port: u16) -> Result<String, BrowserError> {
    let url = format!("http://localhost:{port}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

    let mut last_page_count: Option<usize> = None;
    let mut stable_streak = 0u32;

    // Poll for up to 60s (120 * 500ms) — covers slow machines
    for _ in 0..120 {
        tokio::time::sleep(Duration::from_millis(500)).await;

        let response = match client.get(&url).send().await {
            Ok(r) => r,
            Err(_) => { last_page_count = None; stable_streak = 0; continue; }
        };

        let targets: Vec<serde_json::Value> = match response.json().await {
            Ok(t) => t,
            Err(_) => { last_page_count = None; stable_streak = 0; continue; }
        };

        let page_count = targets.iter()
            .filter(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
            .count();

        if page_count == 0 {
            last_page_count = None;
            stable_streak = 0;
            continue;
        }

        // Check if target count is stable (same as last check)
        if last_page_count == Some(page_count) {
            stable_streak += 1;
        } else {
            stable_streak = 1;
        }
        last_page_count = Some(page_count);

        // Stable for 2 consecutive checks (1s) — good to go
        if stable_streak >= 2 {
            // Grab the first page target
            for target in &targets {
                if target.get("type").and_then(|t| t.as_str()) == Some("page") {
                    if let Some(ws_url) = target.get("webSocketDebuggerUrl").and_then(|u| u.as_str()) {
                        tracing::info!("CDP stable ({page_count} page targets): {ws_url}");
                        return Ok(ws_url.to_owned());
                    }
                }
            }
        }
    }

    Err(BrowserError::Timeout)
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
            BrowserError::Timeout => write!(f, "Browser did not become ready within 60 seconds"),
        }
    }
}

impl std::error::Error for BrowserError {}
