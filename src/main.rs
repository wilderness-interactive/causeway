mod browser;
mod cdp;
mod commands;
mod config;
mod server;

use std::sync::Arc;
use rmcp::ServiceExt;
use cdp::LiveConnection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter("causeway=debug")
        .init();

    // Config search: cwd → source root (two up from exe in target/debug/) → next to exe
    let config_path = [
        Some(std::path::PathBuf::from("causeway.toml")),
        std::env::current_exe().ok().and_then(|p| p.parent()?.parent()?.parent().map(|d| d.join("causeway.toml"))),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("causeway.toml"))),
    ]
    .into_iter()
    .flatten()
    .find(|p| p.exists())
    .unwrap_or_else(|| std::path::PathBuf::from("causeway.toml"));
    let config = config::load_config(config_path.to_str().unwrap_or("causeway.toml"))?;
    tracing::info!("Causeway loaded config: {:?}", config.browser);

    let launch_result = browser::launch(&config.browser).await?;
    let (child, ws_url) = match launch_result {
        browser::LaunchResult::Spawned { child, ws_url } => {
            tracing::info!("Launched new browser, CDP at: {ws_url}");
            (Some(child), ws_url)
        }
        browser::LaunchResult::Existing { ws_url } => {
            tracing::info!("Connected to existing browser, CDP at: {ws_url}");
            (None, ws_url)
        }
    };

    let conn = cdp::connect_to_target(&ws_url).await?;
    tracing::info!("CDP WebSocket connected, domains enabled");

    let live = Arc::new(LiveConnection::new(conn));
    let mcp_server = server::CausewayServer::new(live, config.browser.port);
    mcp_server.resubscribe_events().await;

    let service = mcp_server
        .serve(rmcp::transport::io::stdio())
        .await
        .inspect_err(|e| tracing::error!("Causeway MCP error: {e}"))?;

    tracing::info!("Causeway running on stdio");
    service.waiting().await?;

    browser::shutdown(child);
    Ok(())
}
