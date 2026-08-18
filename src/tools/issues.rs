//! Issue-domain MCP tools: search, read, create, update, delete issues, and
//! list an issue's links to other issues.
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
pub struct SearchIssuesRequest {
    /// YouTrack search query, e.g. `project: DEMO #Unresolved`.
    pub query: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIssueRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateIssueRequest {
    /// Short name or id of the project to create the issue in.
    pub project_id: String,
    /// Issue summary (title).
    pub summary: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateIssueRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteIssueRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListLinksRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[tool_router(router = issues_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "Search issues using YouTrack query syntax.")]
    async fn search_issues(&self, _params: Parameters<SearchIssuesRequest>) -> CallToolResult {
        stub_result("search_issues")
    }

    #[tool(description = "Get a single issue by id.")]
    async fn get_issue(&self, _params: Parameters<GetIssueRequest>) -> CallToolResult {
        stub_result("get_issue")
    }

    #[tool(description = "Create a new issue in a project.")]
    async fn create_issue(&self, _params: Parameters<CreateIssueRequest>) -> CallToolResult {
        stub_result("create_issue")
    }

    #[tool(description = "Update fields on an existing issue.")]
    async fn update_issue(&self, _params: Parameters<UpdateIssueRequest>) -> CallToolResult {
        stub_result("update_issue")
    }

    #[tool(description = "Delete an issue.")]
    async fn delete_issue(&self, _params: Parameters<DeleteIssueRequest>) -> CallToolResult {
        stub_result("delete_issue")
    }

    #[tool(description = "List an issue's links to other issues.")]
    async fn list_links(&self, _params: Parameters<ListLinksRequest>) -> CallToolResult {
        stub_result("list_links")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::issues_router()
}
