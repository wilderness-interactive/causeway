use std::process::Command;
use std::time::Duration;

use crate::config::BrowserConfig;

/// Ensure a browser with CDP is running. Connects to an existing one if the
/// debugging port is live, otherwise spawns a fresh instance. The caller
/// discovers the WebSocket URL separately via browser_ws_url / find_target_ws_url.
pub async fn launch(config: &BrowserConfig) -> Result<(), BrowserError> {
    // Check if CDP is already available (browser already running with debugging port)
    if try_connect_existing(config.port).await.is_ok() {
        tracing::info!("Found existing browser with CDP on port {}", config.port);
        return Ok(());
    }

    // If we got here, CDP isn't available on the port. Chromium ignores
    // --remote-debugging-port when piggybacking on an existing process (even
    // background processes with no visible window). Kill them so the fresh
    // spawn gets the flag. Safe for other Causeway instances: if any had CDP
    // active, try_connect_existing above would have already connected.
    let exe_name = extract_exe_name(&config.executable);
    let match_key = kill_match_key(&config, &exe_name);
    if is_process_running(&match_key) {
        tracing::info!("Killing existing {exe_name} — CDP unavailable, must relaunch with debugging port");
        kill_and_wait(&match_key).await?;
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
    poll_until_stable(config.port).await?;
    Ok(())
}

/// Extract just the executable filename from a full path (e.g. "brave.exe" from the full path)
fn extract_exe_name(executable: &str) -> String {
    std::path::Path::new(executable)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("brave.exe")
        .to_owned()
}

/// The key used to match an existing browser process before killing it. On Unix,
/// pgrep/pkill match the full command line, so a dedicated profile's user-data-dir
/// is a precise, safe key that never tears down a normally-running browser. On
/// Windows, tasklist/taskkill match by image name only, so the exe name is the
/// only workable key there.
#[cfg(not(target_os = "windows"))]
fn kill_match_key(config: &BrowserConfig, exe_name: &str) -> String {
    if config.dedicated_profile {
        if let Some(dir) = &config.user_data_dir {
            return dir.clone();
        }
    }
    exe_name.to_owned()
}

#[cfg(target_os = "windows")]
fn kill_match_key(_config: &BrowserConfig, exe_name: &str) -> String {
    exe_name.to_owned()
}

/// Check if a process matching `match_key` is currently running (Windows: tasklist).
#[cfg(target_os = "windows")]
fn is_process_running(match_key: &str) -> bool {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("IMAGENAME eq {match_key}"), "/NH"])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.contains(match_key)
        }
        Err(_) => false,
    }
}

/// Check if a process matching `match_key` is currently running (Unix: pgrep).
/// `pgrep -f` matches against the full command line, so the executable basename
/// (e.g. "Microsoft Edge") or a dedicated profile's user-data-dir matches the
/// browser and its helper processes.
#[cfg(not(target_os = "windows"))]
fn is_process_running(match_key: &str) -> bool {
    match Command::new("pgrep").args(["-f", match_key]).output() {
        Ok(out) => out.status.success() && !out.stdout.is_empty(),
        Err(_) => false,
    }
}

/// Kill the processes matching `match_key` and wait until they're actually gone.
/// Retries the kill if processes survive, because Chromium spawns many child
/// processes that can respawn or linger (crashpad, updater, GPU process).
async fn kill_and_wait(match_key: &str) -> Result<(), BrowserError> {
    // Graceful first: SIGTERM lets the browser flush its profile (cookies, login
    // state) to disk before it dies, so a relaunch doesn't drop signed-in
    // sessions. Escalate to SIGKILL only for child processes that linger.
    kill_browser_processes(match_key, false);

    for tick in 0..120 {
        if !is_process_running(match_key) {
            tracing::info!("{match_key} fully terminated");
            return Ok(());
        }

        // Force-kill survivors after ~8s of grace, then every 2s.
        if tick >= 32 && tick % 8 == 0 {
            tracing::info!("force-killing remaining {match_key} processes");
            kill_browser_processes(match_key, true);
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Err(BrowserError::LaunchFailed(
        format!("Could not kill {match_key} after 30s — is another program holding it?")
    ))
}

/// Kill processes matching `match_key` (Windows: taskkill). `force` escalates
/// from a polite close request to a hard /F kill.
#[cfg(target_os = "windows")]
fn kill_browser_processes(match_key: &str, force: bool) {
    let mut cmd = Command::new("taskkill");
    cmd.arg("/IM").arg(match_key);
    if force {
        cmd.arg("/F");
    }
    match cmd.output() {
        Ok(o) if !o.status.success() => {
            tracing::warn!("taskkill: {}", String::from_utf8_lossy(&o.stderr).trim());
        }
        Err(e) => tracing::warn!("taskkill error: {e}"),
        Ok(_) => {}
    }
}

/// Kill processes matching `match_key` (Unix: pkill). `pkill -f` matches the key
/// anywhere in the command line, catching Chromium's helper processes (GPU,
/// renderer, crashpad) too. Exit code 1 just means "nothing matched" — not an
/// error worth logging. `force` escalates from SIGTERM (polite, flushes the
/// profile to disk) to SIGKILL (hard).
#[cfg(not(target_os = "windows"))]
fn kill_browser_processes(match_key: &str, force: bool) {
    let signal = if force { "-9" } else { "-TERM" };
    match Command::new("pkill").args([signal, "-f", match_key]).output() {
        Ok(o) if !o.status.success() && o.status.code() != Some(1) => {
            tracing::warn!("pkill: {}", String::from_utf8_lossy(&o.stderr).trim());
        }
        Err(e) => tracing::warn!("pkill error: {e}"),
        Ok(_) => {}
    }
}

/// Find the WebSocket URL for a specific target ID, or the first page target if None.
pub async fn find_target_ws_url(port: u16, target_id: Option<&str>) -> Result<String, BrowserError> {
    // 127.0.0.1, not localhost: on Windows localhost resolves to ::1 first, but Edge's
    // DevTools server binds IPv4 only. The IPv6 route doesn't refuse fast — it hangs to
    // timeout, which makes a live port look dead. The literal address skips DNS and ::1.
    let url = format!("http://127.0.0.1:{port}/json");
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

/// Get the browser-level CDP WebSocket URL (for issuing Target.* commands).
/// This endpoint exists as long as the browser is alive, independent of any tab.
pub async fn browser_ws_url(port: u16) -> Result<String, BrowserError> {
    let url = format!("http://127.0.0.1:{port}/json/version");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|e| BrowserError::LaunchFailed(e.to_string()))?;

    let info: serde_json::Value = client
        .get(&url)
        .send()
        .await
        .map_err(|_| BrowserError::Timeout)?
        .json()
        .await
        .map_err(|_| BrowserError::Timeout)?;

    info.get("webSocketDebuggerUrl")
        .and_then(|u| u.as_str())
        .map(|s| s.to_owned())
        .ok_or(BrowserError::Timeout)
}

/// Poll until CDP is available AND page targets have stabilized.
/// Returns the WS URL of the first stable page target.
/// Handles both slow browser launches and session restore target churn.
async fn poll_until_stable(port: u16) -> Result<String, BrowserError> {
    let url = format!("http://127.0.0.1:{port}/json");
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
