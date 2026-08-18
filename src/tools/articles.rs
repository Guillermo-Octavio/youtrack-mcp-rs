//! Knowledge-base article-domain MCP tools.
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
pub struct GetArticleRequest {
    /// Article id.
    pub article_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateArticleRequest {
    /// Article title.
    pub title: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateArticleRequest {
    /// Article id.
    pub article_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteArticleRequest {
    /// Article id.
    pub article_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArticleAttachmentsRequest {
    /// Article id.
    pub article_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadArticleAttachmentRequest {
    /// Article id.
    pub article_id: String,
}

#[tool_router(router = articles_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List all knowledge-base articles.")]
    async fn list_articles(&self) -> CallToolResult {
        stub_result("list_articles")
    }

    #[tool(description = "Get a single knowledge-base article by id.")]
    async fn get_article(&self, _params: Parameters<GetArticleRequest>) -> CallToolResult {
        stub_result("get_article")
    }

    #[tool(description = "Create a new knowledge-base article.")]
    async fn create_article(&self, _params: Parameters<CreateArticleRequest>) -> CallToolResult {
        stub_result("create_article")
    }

    #[tool(description = "Update an existing knowledge-base article.")]
    async fn update_article(&self, _params: Parameters<UpdateArticleRequest>) -> CallToolResult {
        stub_result("update_article")
    }

    #[tool(description = "Delete a knowledge-base article.")]
    async fn delete_article(&self, _params: Parameters<DeleteArticleRequest>) -> CallToolResult {
        stub_result("delete_article")
    }

    #[tool(description = "List the attachments on a knowledge-base article.")]
    async fn list_article_attachments(
        &self,
        _params: Parameters<ListArticleAttachmentsRequest>,
    ) -> CallToolResult {
        stub_result("list_article_attachments")
    }

    #[tool(description = "Upload an attachment to a knowledge-base article.")]
    async fn upload_article_attachment(
        &self,
        _params: Parameters<UploadArticleAttachmentRequest>,
    ) -> CallToolResult {
        stub_result("upload_article_attachment")
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::articles_router()
}
