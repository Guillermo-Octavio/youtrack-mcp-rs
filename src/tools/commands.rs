//! YouTrack command-domain MCP tools: applying command-language actions to
//! one or many issues.
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
pub struct ApplyCommandRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// YouTrack command text, e.g. `State Fixed`.
    pub command: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BulkCommandRequest {
    /// Issue ids or readable ids to apply the command to.
    pub issue_ids: Vec<String>,
    /// YouTrack command text, e.g. `State Fixed`.
    pub command: String,
}

#[tool_router(router = commands_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "Apply a YouTrack command to a single issue.")]
    async fn apply_command(&self, _params: Parameters<ApplyCommandRequest>) -> CallToolResult {
        stub_result("apply_command")
    }

    #[tool(description = "Apply a YouTrack command to multiple issues at once.")]
    async fn bulk_command(&self, _params: Parameters<BulkCommandRequest>) -> CallToolResult {
        stub_result("bulk_command")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::commands_router()
}
