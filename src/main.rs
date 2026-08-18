//! Binary entry point for the YouTrack MCP server.
//!
//! Wires the aggregate [`ToolServer`] (see `youtrack_mcp_rs::tools`) into
//! `rmcp`'s standard stdio transport: MCP clients speak JSON-RPC over this
//! process's stdin/stdout. Tool domain logic (real YouTrack REST calls)
//! lands in a later phase of the `rust-rewrite` change; every tool this
//! binary currently exposes is a discoverable stub.

use rmcp::ServiceExt;
use youtrack_mcp_rs::tools::ToolServer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = ToolServer::new().serve(rmcp::transport::stdio()).await?;
    server.waiting().await?;
    Ok(())
}
