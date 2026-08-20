//! Issue-attachment-domain REST calls: list, upload, and download an
//! issue's attachments.
//!
//! `download_attachment_bytes` is the hardened exception to this crate's
//! usual request pattern: attachment download URLs YouTrack returns are
//! relative to the host root, not to this client's `/api` base (see
//! [`YouTrackClient::root_url`](super::YouTrackClient) — actually a
//! private field; see the module doc on [`super::YouTrackClient`]), and a
//! redirect from that endpoint is only followed when it stays on the
//! same host as the configured `YOUTRACK_URL`, closing off a malicious
//! or misconfigured attachment host from receiving this server's bearer
//! token via a cross-host redirect.

use url::Url;

use crate::client::page::Page;
use crate::client::YouTrackClient;
use crate::error::YouTrackError;
use crate::youtrack::attachment::IssueAttachment;

/// `fields` projection for [`YouTrackClient::list_attachments`] and
/// [`YouTrackClient::get_attachment`].
const ATTACHMENT_FIELDS: &str = "id,name,mimeType,size,url";

/// Maximum number of bytes [`YouTrackClient::download_attachment_bytes`]
/// will buffer for one attachment download, enforced incrementally as
/// the response body streams in (a remote response has no filesystem
/// metadata to check up front, unlike an upload's local file). Mirrors
/// the 50 MiB cap `upload_attachment`/`upload_article_attachment` enforce
/// on local files: the attachments this server handles are documents,
/// images, and small archives, and 50 MiB comfortably covers that while
/// bounding memory use and download time for one MCP call.
pub const MAX_DOWNLOAD_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum number of redirect hops [`YouTrackClient::download_attachment_bytes`]
/// will follow before giving up. Bounds an otherwise-unbounded redirect
/// chain; YouTrack's own attachment download endpoint redirects at most
/// once in practice (to a signed file URL).
const MAX_DOWNLOAD_REDIRECTS: u32 = 5;

impl YouTrackClient {
    /// Lists the attachments on an issue
    /// (`GET /issues/{issue_id}/attachments`), paginated via
    /// `limit`/`skip`.
    pub async fn list_attachments(
        &self,
        issue_id: &str,
        limit: usize,
        skip: usize,
    ) -> Result<Page<IssueAttachment>, YouTrackError> {
        let path = format!("issues/{issue_id}/attachments");
        self.get_page(&path, ATTACHMENT_FIELDS, vec![], limit, skip)
            .await
    }

    /// Fetches a single attachment's metadata, including its download
    /// `url` (`GET /issues/{issue_id}/attachments/{attachment_id}`).
    pub async fn get_attachment(
        &self,
        issue_id: &str,
        attachment_id: &str,
    ) -> Result<IssueAttachment, YouTrackError> {
        let path = format!("issues/{issue_id}/attachments/{attachment_id}");
        let query = [("fields", ATTACHMENT_FIELDS.to_string())];
        self.get_json(&path, &query, Some(("attachment", attachment_id)))
            .await
    }

    /// Uploads `bytes` as a new attachment named `file_name` on an issue
    /// (`POST /issues/{issue_id}/attachments`, multipart).
    pub async fn upload_attachment(
        &self,
        issue_id: &str,
        file_name: &str,
        mime_type: &str,
        bytes: Vec<u8>,
    ) -> Result<IssueAttachment, YouTrackError> {
        let path = format!("issues/{issue_id}/attachments");
        let query = [("fields", ATTACHMENT_FIELDS.to_string())];
        let file = crate::client::MultipartFile {
            field_name: "file",
            file_name,
            mime_type,
            bytes,
        };
        self.post_multipart(&path, &query, file, Some(("issue", issue_id)))
            .await
    }

    /// Downloads an attachment's bytes from `relative_url` (the `url`
    /// field [`YouTrackClient::get_attachment`] returned), joined onto
    /// this client's host root rather than its `/api` base.
    ///
    /// Hardening, in order:
    /// 1. Uses a dedicated download HTTP client that never follows
    ///    redirects automatically.
    /// 2. On a `3xx` response, resolves the `Location` header against
    ///    the current URL and compares its host against the configured
    ///    `YOUTRACK_URL`'s host. A same-host redirect is followed (up to
    ///    [`MAX_DOWNLOAD_REDIRECTS`] hops); a cross-host redirect is
    ///    rejected with [`YouTrackError::CrossHostRedirect`] and never
    ///    sent to, closing off a path for this server's bearer token to
    ///    leak to an unexpected origin.
    /// 3. Streams the successful response body and rejects with
    ///    [`YouTrackError::DownloadTooLarge`] the moment more than
    ///    [`MAX_DOWNLOAD_BYTES`] have been received, instead of buffering
    ///    an unbounded body first.
    pub async fn download_attachment_bytes(
        &self,
        relative_url: &str,
    ) -> Result<Vec<u8>, YouTrackError> {
        let allowed_host = Url::parse(&self.root_url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_string));
        let mut current_url = format!("{}{relative_url}", self.root_url);

        for _ in 0..=MAX_DOWNLOAD_REDIRECTS {
            let mut response = self
                .download_http
                .get(&current_url)
                .bearer_auth(&self.token)
                .send()
                .await?;
            let status = response.status();

            if status.is_redirection() {
                let location = response
                    .headers()
                    .get(reqwest::header::LOCATION)
                    .and_then(|value| value.to_str().ok())
                    .ok_or_else(|| YouTrackError::Validation {
                        field: None,
                        message: "attachment download redirected with no Location header"
                            .to_string(),
                    })?
                    .to_string();

                let next_url = Url::parse(&current_url)
                    .and_then(|base| base.join(&location))
                    .map_err(|_| YouTrackError::Validation {
                        field: None,
                        message: format!(
                            "attachment download redirected to an invalid URL: {location}"
                        ),
                    })?;

                let next_host = next_url.host_str().map(str::to_string);
                if next_host != allowed_host {
                    return Err(YouTrackError::CrossHostRedirect {
                        location: next_url.to_string(),
                    });
                }

                current_url = next_url.to_string();
                continue;
            }

            if !status.is_success() {
                return Err(match status.as_u16() {
                    401 => YouTrackError::Unauthorized,
                    403 => YouTrackError::Forbidden,
                    other => super::api_error(other, response.text().await.ok()),
                });
            }

            let mut buffer = Vec::new();
            while let Some(chunk) = response.chunk().await? {
                if buffer.len() + chunk.len() > MAX_DOWNLOAD_BYTES as usize {
                    return Err(YouTrackError::DownloadTooLarge {
                        max_bytes: MAX_DOWNLOAD_BYTES,
                    });
                }
                buffer.extend_from_slice(&chunk);
            }
            return Ok(buffer);
        }

        Err(YouTrackError::TooManyRedirects {
            max_redirects: MAX_DOWNLOAD_REDIRECTS,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_client(base_url: &str) -> YouTrackClient {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        YouTrackClient::new(&config).unwrap()
    }

    #[tokio::test]
    async fn list_attachments_deserializes_name_and_size() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments"))
            .and(query_param("$top", "6"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "id": "att-1",
                    "name": "screenshot.png",
                    "mimeType": "image/png",
                    "size": 2048,
                    "url": "/api/files/att-1?sign=abc",
                }])),
            )
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let page = client.list_attachments("DEMO-1", 5, 0).await.unwrap();

        assert_eq!(page.items.len(), 1);
        assert_eq!(page.items[0].name.as_deref(), Some("screenshot.png"));
        assert_eq!(page.items[0].size, Some(2048));
    }

    #[tokio::test]
    async fn get_attachment_deserializes_the_download_url() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "att-1",
                "name": "screenshot.png",
                "url": "/api/files/att-1?sign=abc",
            })))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let attachment = client.get_attachment("DEMO-1", "att-1").await.unwrap();

        assert_eq!(attachment.url.as_deref(), Some("/api/files/att-1?sign=abc"));
    }

    #[tokio::test]
    async fn get_attachment_maps_404_to_not_found_with_the_attachment_id() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/attachments/att-missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .get_attachment("DEMO-1", "att-missing")
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "attachment");
                assert_eq!(id, "att-missing");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn upload_attachment_sends_a_multipart_request_with_the_file_part() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/attachments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "att-1",
                "name": "notes.txt",
            })))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let attachment = client
            .upload_attachment("DEMO-1", "notes.txt", "text/plain", b"hello world".to_vec())
            .await
            .unwrap();

        assert_eq!(attachment.name.as_deref(), Some("notes.txt"));
        let requests = server.received_requests().await.unwrap();
        let body_text = String::from_utf8_lossy(&requests[0].body).to_string();
        assert!(body_text.contains("notes.txt"));
        assert!(body_text.contains("hello world"));
    }

    #[tokio::test]
    async fn upload_attachment_maps_404_to_not_found_with_the_issue_id() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-404/attachments"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .upload_attachment("DEMO-404", "notes.txt", "text/plain", b"hi".to_vec())
            .await
            .unwrap_err();

        match error {
            YouTrackError::NotFound { resource, id } => {
                assert_eq!(resource, "issue");
                assert_eq!(id, "DEMO-404");
            }
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn download_attachment_bytes_fetches_the_body_from_the_host_root_not_the_api_base() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .and(header("authorization", "Bearer test-token"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"file bytes".to_vec()))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let bytes = client
            .download_attachment_bytes("/files/att-1")
            .await
            .unwrap();

        assert_eq!(bytes, b"file bytes");
    }

    #[tokio::test]
    async fn download_attachment_bytes_follows_a_same_host_redirect() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .respond_with(ResponseTemplate::new(302).insert_header("Location", "/signed/att-1"))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/signed/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"redirected bytes".to_vec()))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let bytes = client
            .download_attachment_bytes("/files/att-1")
            .await
            .unwrap();

        assert_eq!(bytes, b"redirected bytes");
    }

    #[tokio::test]
    async fn download_attachment_bytes_rejects_a_cross_host_redirect() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .respond_with(
                ResponseTemplate::new(302)
                    .insert_header("Location", "https://evil.example.com/steal"),
            )
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .download_attachment_bytes("/files/att-1")
            .await
            .unwrap_err();

        assert!(matches!(error, YouTrackError::CrossHostRedirect { .. }));
    }

    #[tokio::test]
    async fn download_attachment_bytes_rejects_a_body_over_the_size_cap() {
        let server = MockServer::start().await;
        let oversized = vec![0u8; (MAX_DOWNLOAD_BYTES + 1) as usize];
        Mock::given(method("GET"))
            .and(path("/files/att-1"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(oversized))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .download_attachment_bytes("/files/att-1")
            .await
            .unwrap_err();

        assert!(matches!(error, YouTrackError::DownloadTooLarge { .. }));
    }

    #[tokio::test]
    async fn download_attachment_bytes_maps_404_to_a_generic_api_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/files/att-missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let client = test_client(&server.uri());

        let error = client
            .download_attachment_bytes("/files/att-missing")
            .await
            .unwrap_err();

        assert!(matches!(error, YouTrackError::Api { status: 404, .. }));
    }
}
