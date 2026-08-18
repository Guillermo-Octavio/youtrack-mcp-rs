//! Issue-activity-domain MCP tools: the audit trail of field changes on an
//! issue.
//!
//! The handler below is a Phase 2 stub: it accepts a minimal typed input
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
pub struct GetIssueActivityRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[tool_router(router = activity_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "Get the field-change activity log for an issue.")]
    async fn get_issue_activity(
        &self,
        _params: Parameters<GetIssueActivityRequest>,
    ) -> CallToolResult {
        stub_result("get_issue_activity")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::activity_router()
}
