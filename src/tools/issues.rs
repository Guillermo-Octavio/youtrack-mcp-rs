//! Issue-domain MCP tools: search, read, create, update, delete issues, and
//! list an issue's links to other issues.
//!
//! `search_issues`, `get_issue`, and `list_links` call the real YouTrack
//! REST API. `create_issue`, `update_issue`, and `delete_issue` remain
//! Phase 2 stubs and land in a later phase of the `rust-rewrite` change.

// Request struct fields aren't read yet by the still-stubbed handlers;
// later phases will consume them.
#![allow(dead_code)]

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::client::issues::ISSUE_FIELDS;
use crate::render::IssueLinks;

use super::{
    cursor_skip, stub_result, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT,
};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchIssuesRequest {
    /// YouTrack search query, e.g. `project: DEMO #Unresolved`.
    pub query: String,
    /// Maximum number of issues to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
    /// YouTrack `fields` projection override. Defaults to a curated set
    /// covering summary, project, reporter, and custom fields.
    #[serde(default)]
    pub fields: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetIssueRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// YouTrack `fields` projection override. Defaults to a curated set
    /// covering summary, project, reporter, and custom fields.
    #[serde(default)]
    pub fields: Option<String>,
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
    #[tool(
        description = "Search issues using YouTrack query syntax. Supports pagination via `cursor`."
    )]
    async fn search_issues(&self, params: Parameters<SearchIssuesRequest>) -> CallToolResult {
        let SearchIssuesRequest {
            query,
            limit,
            cursor,
            fields,
        } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };
        let fields = fields.unwrap_or_else(|| ISSUE_FIELDS.to_string());

        match self
            .client
            .search_issues(&query, &fields, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "Get a single issue by id.")]
    async fn get_issue(&self, params: Parameters<GetIssueRequest>) -> CallToolResult {
        let GetIssueRequest { issue_id, fields } = params.0;
        let fields = fields.unwrap_or_else(|| ISSUE_FIELDS.to_string());

        match self.client.get_issue(&issue_id, &fields).await {
            Ok(issue) => tool_ok_result(&issue),
            Err(error) => tool_error_result(error.into()),
        }
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
    async fn list_links(&self, params: Parameters<ListLinksRequest>) -> CallToolResult {
        let ListLinksRequest { issue_id } = params.0;

        match self.client.list_links(&issue_id).await {
            Ok(links) => tool_ok_result(&IssueLinks { issue_id, links }),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::issues_router()
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
    async fn search_issues_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "idReadable": "DEMO-1", "summary": "Fix the thing" }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .search_issues(Parameters(SearchIssuesRequest {
                query: "project: DEMO".to_string(),
                limit: None,
                cursor: None,
                fields: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["idReadable"], "DEMO-1");
    }

    #[tokio::test]
    async fn search_issues_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .search_issues(Parameters(SearchIssuesRequest {
                query: "project: DEMO".to_string(),
                limit: None,
                cursor: Some("not-a-valid-cursor".to_string()),
                fields: None,
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "validation");
    }

    #[tokio::test]
    async fn get_issue_maps_unauthorized_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .get_issue(Parameters(GetIssueRequest {
                issue_id: "DEMO-1".to_string(),
                fields: None,
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "unauthorized");
    }

    #[tokio::test]
    async fn list_links_returns_the_issue_id_alongside_its_links() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/links"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_links(Parameters(ListLinksRequest {
                issue_id: "DEMO-1".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["issue_id"], "DEMO-1");
        assert_eq!(structured["links"].as_array().unwrap().len(), 0);
    }
}
