//! Comment-domain MCP tools.
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
pub struct ListCommentsRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Comment body text.
    pub text: String,
}

#[tool_router(router = comments_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List the comments on an issue.")]
    async fn list_comments(&self, _params: Parameters<ListCommentsRequest>) -> CallToolResult {
        stub_result("list_comments")
    }

    #[tool(description = "Add a comment to an issue.")]
    async fn add_comment(&self, _params: Parameters<AddCommentRequest>) -> CallToolResult {
        stub_result("add_comment")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::comments_router()
}
