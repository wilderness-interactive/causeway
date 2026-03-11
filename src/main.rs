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

    // Lazy init: start MCP server immediately, browser launches on first tool call
    let live = Arc::new(LiveConnection::empty());
    let mcp_server = server::CausewayServer::new(live, config.browser.port, config.browser);

    let service = mcp_server
        .serve(rmcp::transport::io::stdio())
        .await
        .inspect_err(|e| tracing::error!("Causeway MCP error: {e}"))?;

    tracing::info!("Causeway running on stdio (browser will launch on first tool call)");
    service.waiting().await?;

    Ok(())
}
