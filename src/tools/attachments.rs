//! Issue-attachment-domain MCP tools: list, upload, and download an
//! issue's attachments.
//!
//! Every handler calls the real YouTrack REST API. `upload_attachment`
//! enforces the same local file-read security policy as
//! `upload_article_attachment` (see [`super::local_file::read_attachment_file`]).
//! `download_attachment` enforces the matching write-side policy on its
//! caller-supplied destination path (see
//! [`super::local_file::validate_destination_path`]) and never follows a
//! cross-host redirect (see
//! [`crate::client::YouTrackClient::download_attachment_bytes`]).

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::ToolError;
use crate::render::DownloadAttachmentResult;

use super::local_file::{read_attachment_file, validate_destination_path, write_downloaded_file};
use super::{cursor_skip, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAttachmentsRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Maximum number of attachments to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadAttachmentRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Local file path to upload. Must point directly to a regular file:
    /// no `..` traversal segments and no symlinks are accepted, and the
    /// file must be at most 50 MiB.
    pub file_path: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DownloadAttachmentRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Attachment id, as returned by `list_attachments`.
    pub attachment_id: String,
    /// Local destination file path to write the downloaded attachment
    /// to. Must not contain `..` traversal segments and must not already
    /// exist as a symlink.
    pub destination_path: String,
}

#[tool_router(router = attachments_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List the attachments on an issue. Supports pagination via `cursor`.")]
    async fn list_attachments(&self, params: Parameters<ListAttachmentsRequest>) -> CallToolResult {
        let ListAttachmentsRequest {
            issue_id,
            limit,
            cursor,
        } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_attachments(&issue_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Upload a local file as an attachment to an issue. `file_path` must \
                        point directly to a regular file: paths containing `..` traversal \
                        segments and symlinks are rejected, and the file must be at most \
                        50 MiB."
    )]
    async fn upload_attachment(
        &self,
        params: Parameters<UploadAttachmentRequest>,
    ) -> CallToolResult {
        let UploadAttachmentRequest {
            issue_id,
            file_path,
        } = params.0;

        let (file_name, mime_type, bytes) = match read_attachment_file(&file_path) {
            Ok(read) => read,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .upload_attachment(&issue_id, &file_name, &mime_type, bytes)
            .await
        {
            Ok(attachment) => tool_ok_result(&attachment),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Download an issue attachment by id to a local destination path. \
                        `destination_path` must not contain `..` traversal segments and must \
                        not already exist as a symlink. The download is capped at 50 MiB and \
                        never follows a redirect to a different host than the configured \
                        YouTrack server."
    )]
    async fn download_attachment(
        &self,
        params: Parameters<DownloadAttachmentRequest>,
    ) -> CallToolResult {
        let DownloadAttachmentRequest {
            issue_id,
            attachment_id,
            destination_path,
        } = params.0;

        let destination = match validate_destination_path(&destination_path) {
            Ok(destination) => destination,
            Err(error) => return tool_error_result(error),
        };

        let attachment = match self.client.get_attachment(&issue_id, &attachment_id).await {
            Ok(attachment) => attachment,
            Err(error) => return tool_error_result(error.into()),
        };

        let Some(download_url) = attachment.url else {
            return tool_error_result(ToolError {
                code: "attachment_missing_url",
                message: format!(
                    "attachment `{attachment_id}` on issue `{issue_id}` has no download URL"
                ),
                hint: Some(
                    "Try `list_attachments` again; YouTrack may not have finished processing \
                     this attachment yet."
                        .to_string(),
                ),
            });
        };

        let bytes = match self.client.download_attachment_bytes(&download_url).await {
            Ok(bytes) => bytes,
            Err(error) => return tool_error_result(error.into()),
        };

        if let Err(error) = write_downloaded_file(&destination, &bytes) {
            return tool_error_result(error);
        }

        tool_ok_result(&DownloadAttachmentResult {
            issue_id,
            attachment_id,
            destination_path,
            bytes_written: bytes.len() as u64,
        })
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::attachments_router()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::Parameters;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::client::attachments::MAX_DOWNLOAD_BYTES;
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
    async fn list_attachments_returns_a_page_of_attachments() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "att-1", "name": "screenshot.png", "mimeType": "image/png", "size": 1024 }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_attachments(Parameters(ListAttachmentsRequest {
                issue_id: "DEMO-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["name"], "screenshot.png");
    }

    #[tokio::test]
    async fn list_attachments_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .list_attachments(Parameters(ListAttachmentsRequest {
                issue_id: "DEMO-1".to_string(),
                limit: None,
                cursor: Some("not-a-valid-cursor".to_string()),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "validation");
    }

    #[tokio::test]
    async fn upload_attachment_uploads_a_normal_file_successfully() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "id": "att-1", "name": "notes.txt" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let file_path = unique_temp_path("notes.txt");
        fs::write(&file_path, b"hello world").unwrap();

        let result = tool_server
            .upload_attachment(Parameters(UploadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
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
    async fn upload_attachment_rejects_a_path_traversal_attempt() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .upload_attachment(Parameters(UploadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                file_path: "../../etc/passwd".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "path_traversal");
    }

    #[tokio::test]
    async fn upload_attachment_rejects_a_symlinked_source() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let real_path = unique_temp_path("real.txt");
        fs::write(&real_path, b"secret contents").unwrap();
        let link_path = unique_temp_path("link.txt");
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        let result = tool_server
            .upload_attachment(Parameters(UploadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
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
    async fn upload_attachment_rejects_an_oversized_file() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let file_path = unique_temp_path("huge.bin");
        let oversized = vec![0u8; (MAX_ATTACHMENT_SIZE_BYTES + 1) as usize];
        fs::write(&file_path, &oversized).unwrap();

        let result = tool_server
            .upload_attachment(Parameters(UploadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                file_path: file_path.to_str().unwrap().to_string(),
            }))
            .await;

        fs::remove_file(&file_path).ok();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "file_too_large");
    }

    #[tokio::test]
    async fn download_attachment_writes_the_file_to_the_destination_path() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "att-1",
                "name": "screenshot.png",
                "url": "/files/att-1",
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"attachment bytes".to_vec()))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());
        let dest_path = unique_temp_path("downloaded.png");

        let result = tool_server
            .download_attachment(Parameters(DownloadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                attachment_id: "att-1".to_string(),
                destination_path: dest_path.to_str().unwrap().to_string(),
            }))
            .await;

        let written = fs::read(&dest_path);
        fs::remove_file(&dest_path).ok();

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["bytes_written"], 16);
        assert_eq!(written.unwrap(), b"attachment bytes");
    }

    #[tokio::test]
    async fn download_attachment_rejects_a_path_traversal_destination() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .download_attachment(Parameters(DownloadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                attachment_id: "att-1".to_string(),
                destination_path: "../../etc/passwd".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "path_traversal");
    }

    #[tokio::test]
    async fn download_attachment_rejects_a_symlinked_destination() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let real_path = unique_temp_path("real_target.txt");
        fs::write(&real_path, b"existing contents").unwrap();
        let link_path = unique_temp_path("dest_link.txt");
        std::os::unix::fs::symlink(&real_path, &link_path).unwrap();

        let result = tool_server
            .download_attachment(Parameters(DownloadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                attachment_id: "att-1".to_string(),
                destination_path: link_path.to_str().unwrap().to_string(),
            }))
            .await;

        fs::remove_file(&link_path).ok();
        fs::remove_file(&real_path).ok();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "symlink_rejected");
    }

    #[tokio::test]
    async fn download_attachment_rejects_an_oversized_download() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "att-1",
                "name": "huge.bin",
                "url": "/files/att-1",
            })))
            .mount(&server)
            .await;
        let oversized = vec![0u8; (MAX_DOWNLOAD_BYTES + 1) as usize];
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(oversized))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());
        let dest_path = unique_temp_path("huge_download.bin");

        let result = tool_server
            .download_attachment(Parameters(DownloadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                attachment_id: "att-1".to_string(),
                destination_path: dest_path.to_str().unwrap().to_string(),
            }))
            .await;

        let leftover_written = dest_path.exists();
        fs::remove_file(&dest_path).ok();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "download_too_large");
        assert!(
            !leftover_written,
            "oversized download must not be written to disk"
        );
    }

    #[tokio::test]
    async fn download_attachment_rejects_a_cross_host_redirect() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "att-1",
                "name": "screenshot.png",
                "url": "/files/att-1",
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://evil.example.com/steal"),
            )
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());
        let dest_path = unique_temp_path("never_written.bin");

        let result = tool_server
            .download_attachment(Parameters(DownloadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                attachment_id: "att-1".to_string(),
                destination_path: dest_path.to_str().unwrap().to_string(),
            }))
            .await;

        let leftover_written = dest_path.exists();
        fs::remove_file(&dest_path).ok();

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "cross_host_redirect");
        assert!(
            !leftover_written,
            "cross-host redirect must not be written to disk"
        );
    }

    #[tokio::test]
    async fn download_attachment_maps_404_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments/att-missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .download_attachment(Parameters(DownloadAttachmentRequest {
                issue_id: "DEMO-1".to_string(),
                attachment_id: "att-missing".to_string(),
                destination_path: unique_temp_path("never.bin").to_str().unwrap().to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "not_found");
    }
}
