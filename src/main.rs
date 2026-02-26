mod browser;
mod cdp;
mod commands;
mod config;
mod server;

use std::sync::Arc;
use rmcp::ServiceExt;

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

    let conn = cdp::connect(&ws_url).await?;
    tracing::info!("CDP WebSocket connected");

    // Enable required domains
    cdp::execute(&conn, commands::enable_page()).await?;
    cdp::execute(&conn, commands::enable_dom()).await?;
    cdp::execute(&conn, commands::enable_runtime()).await?;
    tracing::info!("CDP domains enabled");

    let conn = Arc::new(conn);
    let mcp_server = server::CausewayServer::new(conn, config.browser.port);

    let service = mcp_server
        .serve(rmcp::transport::io::stdio())
        .await
        .inspect_err(|e| tracing::error!("Causeway MCP error: {e}"))?;

    tracing::info!("Causeway running on stdio");
    service.waiting().await?;

    browser::shutdown(child);
    Ok(())
}
