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

    let config = config::load_config("causeway.toml")?;
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

    let service = mcp_server
        .serve(rmcp::transport::io::stdio())
        .await
        .inspect_err(|e| tracing::error!("Causeway MCP error: {e}"))?;

    tracing::info!("Causeway running on stdio");
    service.waiting().await?;

    browser::shutdown(child);
    Ok(())
}
