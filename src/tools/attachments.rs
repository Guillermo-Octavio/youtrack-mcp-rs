//! Issue-attachment-domain MCP tools.
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
pub struct ListAttachmentsRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadAttachmentRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadAttachmentRequest {
    /// Attachment id.
    pub attachment_id: String,
}

#[tool_router(router = attachments_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List the attachments on an issue.")]
    async fn list_attachments(
        &self,
        _params: Parameters<ListAttachmentsRequest>,
    ) -> CallToolResult {
        stub_result("list_attachments")
    }

    #[tool(description = "Upload an attachment to an issue.")]
    async fn upload_attachment(
        &self,
        _params: Parameters<UploadAttachmentRequest>,
    ) -> CallToolResult {
        stub_result("upload_attachment")
    }

    #[tool(description = "Download an attachment by id.")]
    async fn download_attachment(
        &self,
        _params: Parameters<DownloadAttachmentRequest>,
    ) -> CallToolResult {
        stub_result("download_attachment")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::attachments_router()
}
