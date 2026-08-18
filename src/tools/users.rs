//! User-domain MCP tools: current user, search, and issue watchers.
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
pub struct SearchUsersRequest {
    /// Free-text query matched against name/login/email.
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWatchersRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[tool_router(router = users_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "Get the currently authenticated YouTrack user.")]
    async fn get_current_user(&self) -> CallToolResult {
        stub_result("get_current_user")
    }

    #[tool(description = "Search YouTrack users by name, login, or email.")]
    async fn search_users(&self, _params: Parameters<SearchUsersRequest>) -> CallToolResult {
        stub_result("search_users")
    }

    #[tool(description = "List the users watching an issue.")]
    async fn list_watchers(&self, _params: Parameters<ListWatchersRequest>) -> CallToolResult {
        stub_result("list_watchers")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::users_router()
}
