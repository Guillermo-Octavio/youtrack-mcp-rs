//! Time-tracking-domain MCP tools: work items on issues.
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
pub struct ListWorkItemsRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddWorkItemRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Work item duration, in minutes.
    pub duration_minutes: u32,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteWorkItemRequest {
    /// Work item id.
    pub work_item_id: String,
}

#[tool_router(router = time_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List the time-tracking work items logged on an issue.")]
    async fn list_work_items(&self, _params: Parameters<ListWorkItemsRequest>) -> CallToolResult {
        stub_result("list_work_items")
    }

    #[tool(description = "Log a time-tracking work item on an issue.")]
    async fn add_work_item(&self, _params: Parameters<AddWorkItemRequest>) -> CallToolResult {
        stub_result("add_work_item")
    }

    #[tool(description = "Delete a time-tracking work item.")]
    async fn delete_work_item(&self, _params: Parameters<DeleteWorkItemRequest>) -> CallToolResult {
        stub_result("delete_work_item")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::time_router()
}
