//! Project-metadata-domain MCP tools: projects, custom fields, tags, and
//! saved searches.
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
pub struct ListProjectFieldsRequest {
    /// Short name or id of the project.
    pub project_id: String,
}

#[tool_router(router = metadata_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List all YouTrack projects visible to the current user.")]
    async fn list_projects(&self) -> CallToolResult {
        stub_result("list_projects")
    }

    #[tool(description = "List the custom fields configured on a project.")]
    async fn list_project_fields(
        &self,
        _params: Parameters<ListProjectFieldsRequest>,
    ) -> CallToolResult {
        stub_result("list_project_fields")
    }

    #[tool(description = "List all issue tags visible to the current user.")]
    async fn list_tags(&self) -> CallToolResult {
        stub_result("list_tags")
    }

    #[tool(description = "List all saved searches visible to the current user.")]
    async fn list_saved_searches(&self) -> CallToolResult {
        stub_result("list_saved_searches")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::metadata_router()
}
