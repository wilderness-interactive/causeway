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

    // Config search: local_causeway.toml (personal overrides, gitignored) → causeway.toml (defaults)
    // At each location: cwd → source root (two up from exe in target/debug/) → next to exe
    let search_dirs: Vec<std::path::PathBuf> = [
        Some(std::path::PathBuf::from(".")),
        std::env::current_exe().ok().and_then(|p| p.parent()?.parent()?.parent().map(|d| d.to_path_buf())),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.to_path_buf())),
    ]
    .into_iter()
    .flatten()
    .collect();

    let config_path = search_dirs.iter()
        .map(|d| d.join("local_causeway.toml"))
        .chain(search_dirs.iter().map(|d| d.join("causeway.toml")))
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
