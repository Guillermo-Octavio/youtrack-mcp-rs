//! Agile-board-domain MCP tools: boards and sprints.
//!
//! Every handler below is a Phase 2 stub: it accepts a minimal typed input
//! and always returns [`stub_result`]. Real YouTrack REST calls land in a
//! later phase of the `rust-rewrite` change.

// Request struct fields aren't read yet because every handler ignores its
// input; Phase 3 domain logic will consume them.
#![allow(dead_code)]

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{stub_result, ToolServer};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSprintsRequest {
    /// Agile board id.
    pub board_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSprintRequest {
    /// Sprint id.
    pub sprint_id: String,
}

#[tool_router(router = agile_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List all agile boards.")]
    async fn list_agile_boards(&self) -> CallToolResult {
        stub_result("list_agile_boards")
    }

    #[tool(description = "List the sprints on an agile board.")]
    async fn list_sprints(&self, _params: Parameters<ListSprintsRequest>) -> CallToolResult {
        stub_result("list_sprints")
    }

    #[tool(description = "Get a single sprint by id.")]
    async fn get_sprint(&self, _params: Parameters<GetSprintRequest>) -> CallToolResult {
        stub_result("get_sprint")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::agile_router()
}
