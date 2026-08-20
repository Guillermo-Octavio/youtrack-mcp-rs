//! Comment-domain MCP tools: list an issue's comments and add a new one.
//!
//! Every handler calls the real YouTrack REST API.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{cursor_skip, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListCommentsRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Maximum number of comments to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddCommentRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Comment body text.
    pub text: String,
}

#[tool_router(router = comments_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List the comments on an issue. Supports pagination via `cursor`.")]
    async fn list_comments(&self, params: Parameters<ListCommentsRequest>) -> CallToolResult {
        let ListCommentsRequest {
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
            .list_comments(&issue_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "Add a comment to an issue.")]
    async fn add_comment(&self, params: Parameters<AddCommentRequest>) -> CallToolResult {
        let AddCommentRequest { issue_id, text } = params.0;

        match self.client.add_comment(&issue_id, &text).await {
            Ok(comment) => tool_ok_result(&comment),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::comments_router()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use rmcp::handler::server::wrapper::Parameters;
    use url::Url;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::client::YouTrackClient;
    use crate::config::Config;

    fn test_server(base_url: &str) -> ToolServer {
        let config = Config {
            youtrack_url: Url::parse(base_url).unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        ToolServer::new(Arc::new(YouTrackClient::new(&config).unwrap()))
    }

    #[tokio::test]
    async fn list_comments_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "1", "text": "First comment", "created": 0,
                  "author": { "login": "jdoe", "fullName": "Jane Doe" } }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_comments(Parameters(ListCommentsRequest {
                issue_id: "DEMO-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["text"], "First comment");
    }

    #[tokio::test]
    async fn list_comments_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .list_comments(Parameters(ListCommentsRequest {
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
    async fn list_comments_maps_404_to_a_tool_error() {
        // `list_comments` is a paginated list tool, like `list_articles`/
        // `search_issues`: a 404 surfaces as the generic `api_error` code
        // those tools use, not `not_found`.
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-404/comments"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_comments(Parameters(ListCommentsRequest {
                issue_id: "DEMO-404".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "api_error");
    }

    #[tokio::test]
    async fn add_comment_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/comments"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "id": "1", "text": "Looks good", "created": 0,
                  "author": { "login": "jdoe", "fullName": "Jane Doe" } }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .add_comment(Parameters(AddCommentRequest {
                issue_id: "DEMO-1".to_string(),
                text: "Looks good".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["text"], "Looks good");
    }

    #[tokio::test]
    async fn add_comment_maps_404_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-404/comments"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .add_comment(Parameters(AddCommentRequest {
                issue_id: "DEMO-404".to_string(),
                text: "Looks good".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "not_found");
    }
}
