//! Knowledge-base article-domain MCP tools: list, get, create, update,
//! delete articles, and list/upload an article's attachments.
//!
//! Every handler calls the real YouTrack REST API. `upload_article_attachment`
//! additionally enforces a local file-read security policy (see
//! [`super::local_file::read_attachment_file`]) before ever opening the
//! caller-supplied path. That policy — and the matching one for
//! attachment downloads — is shared with the issue-attachment domain in
//! [`super::local_file`].

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::render::DeleteArticleResult;

use super::local_file::read_attachment_file;
use super::{cursor_skip, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListArticlesRequest {
    /// Short name or internal id of the project whose articles to list.
    pub project_id: String,
    /// Maximum number of articles to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetArticleRequest {
    /// Article id.
    pub article_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateArticleRequest {
    /// Short name or internal id of the project to create the article in.
    pub project_id: String,
    /// Article title.
    pub summary: String,
    /// Article body, in markdown.
    #[serde(default)]
    pub content: Option<String>,
    /// Readable id of an existing article to nest this one under (e.g.
    /// `KB-A-1`). Resolved to its internal id before the request is sent.
    #[serde(default)]
    pub parent_article: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateArticleRequest {
    /// Article id.
    pub article_id: String,
    /// New article title. Omit to leave unchanged.
    #[serde(default)]
    pub summary: Option<String>,
    /// New article body, in markdown. Omit to leave unchanged.
    #[serde(default)]
    pub content: Option<String>,
    /// Readable id of another article to move this one under. Resolved to
    /// its internal id before the request is sent. Omit to leave unchanged.
    #[serde(default)]
    pub parent_article: Option<String>,
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
    /// Maximum number of attachments to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadArticleAttachmentRequest {
    /// Article id.
    pub article_id: String,
    /// Local file path to upload. Must point directly to a regular file:
    /// no `..` traversal segments and no symlinks are accepted, and the
    /// file must be at most 50 MiB.
    pub file_path: String,
}

#[tool_router(router = articles_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(
        description = "List a project's knowledge-base articles. Supports pagination via \
                           `cursor`."
    )]
    async fn list_articles(&self, params: Parameters<ListArticlesRequest>) -> CallToolResult {
        let ListArticlesRequest {
            project_id,
            limit,
            cursor,
        } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_articles(&project_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "Get a single knowledge-base article by id, including its content.")]
    async fn get_article(&self, params: Parameters<GetArticleRequest>) -> CallToolResult {
        let GetArticleRequest { article_id } = params.0;

        match self.client.get_article(&article_id).await {
            Ok(article) => tool_ok_result(&article),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Create a new knowledge-base article in a project. `parent_article` \
                        nests it under an existing article, by that article's readable id."
    )]
    async fn create_article(&self, params: Parameters<CreateArticleRequest>) -> CallToolResult {
        let CreateArticleRequest {
            project_id,
            summary,
            content,
            parent_article,
        } = params.0;

        match self
            .client
            .create_article(
                &project_id,
                &summary,
                content.as_deref(),
                parent_article.as_deref(),
            )
            .await
        {
            Ok(article) => tool_ok_result(&article),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Update the summary, content, and/or parent article of an existing \
                        knowledge-base article. Only the values passed are changed; omit a \
                        field to leave it as-is."
    )]
    async fn update_article(&self, params: Parameters<UpdateArticleRequest>) -> CallToolResult {
        let UpdateArticleRequest {
            article_id,
            summary,
            content,
            parent_article,
        } = params.0;

        match self
            .client
            .update_article(
                &article_id,
                summary.as_deref(),
                content.as_deref(),
                parent_article.as_deref(),
            )
            .await
        {
            Ok(article) => tool_ok_result(&article),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Permanently delete a knowledge-base article. THIS IS IRREVERSIBLE: \
                        YouTrack has no trash for articles, so the article, its content, and \
                        any sub-articles are lost with no way to recover them. Only call this \
                        with explicit user confirmation."
    )]
    async fn delete_article(&self, params: Parameters<DeleteArticleRequest>) -> CallToolResult {
        let DeleteArticleRequest { article_id } = params.0;

        match self.client.delete_article(&article_id).await {
            Ok(()) => tool_ok_result(&DeleteArticleResult {
                article_id,
                deleted: true,
            }),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "List the attachments on a knowledge-base article. Supports \
                           pagination via `cursor`."
    )]
    async fn list_article_attachments(
        &self,
        params: Parameters<ListArticleAttachmentsRequest>,
    ) -> CallToolResult {
        let ListArticleAttachmentsRequest {
            article_id,
            limit,
            cursor,
        } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_article_attachments(&article_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Upload a local file as an attachment to a knowledge-base article. \
                        `file_path` must point directly to a regular file: paths containing \
                        `..` traversal segments and symlinks are rejected, and the file must \
                        be at most 50 MiB."
    )]
    async fn upload_article_attachment(
        &self,
        params: Parameters<UploadArticleAttachmentRequest>,
    ) -> CallToolResult {
        let UploadArticleAttachmentRequest {
            article_id,
            file_path,
        } = params.0;

        let (file_name, mime_type, bytes) = match read_attachment_file(&file_path) {
            Ok(read) => read,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .upload_article_attachment(&article_id, &file_name, &mime_type, bytes)
            .await
        {
            Ok(attachment) => tool_ok_result(&attachment),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::articles_router()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::Parameters;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use std::fs;

    use super::*;
    use crate::client::YouTrackClient;
    use crate::config::Config;
    use crate::tools::local_file::MAX_ATTACHMENT_SIZE_BYTES;

    fn test_server(base_url: &str) -> ToolServer {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        ToolServer::new(Arc::new(YouTrackClient::new(&config).unwrap()))
    }

    fn unique_temp_path(suffix: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "youtrack_mcp_rs_test_{}_{}_{suffix}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[tokio::test]
    async fn list_articles_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "idReadable": "KB-A-1", "summary": "Runbook" }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_articles(Parameters(ListArticlesRequest {
                project_id: "0-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["idReadable"], "KB-A-1");
    }

    #[tokio::test]
    async fn list_articles_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .list_articles(Parameters(ListArticlesRequest {
                project_id: "0-1".to_string(),
                limit: None,
                cursor: Some("not-a-valid-cursor".to_string()),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "validation");
    }

    #[tokio::test]
    async fn get_article_returns_a_successful_result_with_content() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "idReadable": "KB-A-1", "summary": "Runbook", "content": "# Steps" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .get_article(Parameters(GetArticleRequest {
                article_id: "KB-A-1".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["content"], "# Steps");
    }

    #[tokio::test]
    async fn get_article_maps_404_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .get_article(Parameters(GetArticleRequest {
                article_id: "KB-A-404".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "not_found");
    }

    #[tokio::test]
    async fn create_article_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "idReadable": "KB-A-1", "summary": "Runbook" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .create_article(Parameters(CreateArticleRequest {
                project_id: "0-1".to_string(),
                summary: "Runbook".to_string(),
                content: None,
                parent_article: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["idReadable"], "KB-A-1");
    }

    #[tokio::test]
    async fn update_article_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles/KB-A-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "idReadable": "KB-A-1", "summary": "New summary" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .update_article(Parameters(UpdateArticleRequest {
                article_id: "KB-A-1".to_string(),
                summary: Some("New summary".to_string()),
                content: None,
                parent_article: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["summary"], "New summary");
    }

    #[tokio::test]
    async fn delete_article_returns_a_successful_result_confirming_the_deleted_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/articles/KB-A-1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .delete_article(Parameters(DeleteArticleRequest {
                article_id: "KB-A-1".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["article_id"], "KB-A-1");
        assert_eq!(structured["deleted"], true);
    }

    #[tokio::test]
    async fn delete_article_maps_404_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/articles/KB-A-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .delete_article(Parameters(DeleteArticleRequest {
                article_id: "KB-A-404".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "not_found");
    }

    #[tokio::test]
    async fn list_article_attachments_returns_a_page_of_attachments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/articles/KB-A-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "att-1", "name": "diagram.png", "mimeType": "image/png", "size": 512 }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_article_attachments(Parameters(ListArticleAttachmentsRequest {
                article_id: "KB-A-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["name"], "diagram.png");
    }

    #[tokio::test]
    async fn upload_article_attachment_uploads_a_normal_file_successfully() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/articles/KB-A-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "id": "att-1", "name": "notes.txt" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let file_path = unique_temp_path("notes.txt");
        fs::write(&file_path, b"hello world").unwrap();

        let result = tool_server
            .upload_article_attachment(Parameters(UploadArticleAttachmentRequest {
                article_id: "KB-A-1".to_string(),
                file_path: file_path.to_str().unwrap().to_string(),
            }))
            .await;

        fs::remove_file(&file_path).ok();

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["name"], "notes.txt");
        let requests = server.received_requests().await.unwrap();
        let body_text = String::from_utf8_lossy(&requests[0].body).to_string();
        assert!(body_text.contains("hello world"));
    }

    #[tokio::test]
    async fn upload_article_attachment_rejects_a_path_traversal_attempt() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .upload_article_attachment(Parameters(UploadArticleAttachmentRequest {
                article_id: "KB-A-1".to_string(),
                file_path: "../../etc/passwd".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "path_traversal");
    }

    #[tokio::test]
    async fn upload_article_attachment_rejects_a_symlinked_destination() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let real_path = unique_temp_path("real.txt");
        fs::write(&real_path, b"secret contents").unwrap();
        let link_path = unique_temp_path("link.txt");
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        let result = tool_server
            .upload_article_attachment(Parameters(UploadArticleAttachmentRequest {
                article_id: "KB-A-1".to_string(),
                file_path: link_path.to_str().unwrap().to_string(),
            }))
            .await;

        fs::remove_file(&link_path).ok();
        fs::remove_file(&real_path).ok();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "symlink_rejected");
    }

    #[tokio::test]
    async fn upload_article_attachment_rejects_an_oversized_file() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let file_path = unique_temp_path("huge.bin");
        let oversized = vec![0u8; (MAX_ATTACHMENT_SIZE_BYTES + 1) as usize];
        fs::write(&file_path, &oversized).unwrap();

        let result = tool_server
            .upload_article_attachment(Parameters(UploadArticleAttachmentRequest {
                article_id: "KB-A-1".to_string(),
                file_path: file_path.to_str().unwrap().to_string(),
            }))
            .await;

        fs::remove_file(&file_path).ok();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "file_too_large");
    }

    #[tokio::test]
    async fn upload_article_attachment_rejects_a_missing_file() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .upload_article_attachment(Parameters(UploadArticleAttachmentRequest {
                article_id: "KB-A-1".to_string(),
                file_path: unique_temp_path("does-not-exist.txt")
                    .to_str()
                    .unwrap()
                    .to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "file_not_found");
    }
}
