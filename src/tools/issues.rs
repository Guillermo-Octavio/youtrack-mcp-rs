//! Issue-domain MCP tools: search, read, create, update, delete issues, and
//! list an issue's links to other issues.
//!
//! Every tool in this module calls the real YouTrack REST API.
//! `create_issue` and `update_issue` accept an open-ended `fields` map of
//! custom-field values, validated against the target project's
//! configured custom fields before any request is sent.

use std::collections::HashMap;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::client::issues::ISSUE_FIELDS;
use crate::render::{DeleteIssueResult, IssueLinks};

use super::{cursor_skip, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT};

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
    /// Short name or internal id of the project to create the issue in.
    pub project_id: String,
    /// Issue summary (title).
    pub summary: String,
    /// Issue description.
    #[serde(default)]
    pub description: Option<String>,
    /// Custom field values, keyed by the field's name (e.g.
    /// `{"Priority": "Critical"}`). Validated against the project's
    /// configured custom fields before the issue is created; an
    /// unrecognized field name fails with `unknown_custom_field` and
    /// creates nothing.
    #[serde(default)]
    pub fields: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct UpdateIssueRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// New issue summary. Omit to leave unchanged.
    #[serde(default)]
    pub summary: Option<String>,
    /// New issue description. Omit to leave unchanged.
    #[serde(default)]
    pub description: Option<String>,
    /// Custom field values, keyed by the field's name. Validated against
    /// the issue's project's configured custom fields before the update
    /// is sent; an unrecognized field name fails with
    /// `unknown_custom_field` and updates nothing.
    #[serde(default)]
    pub fields: Option<HashMap<String, serde_json::Value>>,
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

    #[tool(
        description = "Create a new issue in a project. `fields` accepts custom field values \
                        keyed by field name (e.g. `{\"Priority\": \"Critical\"}`), validated \
                        against the project's configured custom fields; use \
                        `list_project_fields` first to see the valid names and values."
    )]
    async fn create_issue(&self, params: Parameters<CreateIssueRequest>) -> CallToolResult {
        let CreateIssueRequest {
            project_id,
            summary,
            description,
            fields,
        } = params.0;

        match self
            .client
            .create_issue(
                &project_id,
                &summary,
                description.as_deref(),
                fields.as_ref(),
            )
            .await
        {
            Ok(issue) => tool_ok_result(&issue),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Update the summary, description, and/or custom fields on an existing \
                        issue. Only the values passed are changed; omit a field to leave it \
                        as-is. `fields` is validated against the issue's project's configured \
                        custom fields the same way as `create_issue`."
    )]
    async fn update_issue(&self, params: Parameters<UpdateIssueRequest>) -> CallToolResult {
        let UpdateIssueRequest {
            issue_id,
            summary,
            description,
            fields,
        } = params.0;

        match self
            .client
            .update_issue(
                &issue_id,
                summary.as_deref(),
                description.as_deref(),
                fields.as_ref(),
            )
            .await
        {
            Ok(issue) => tool_ok_result(&issue),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Permanently delete an issue. THIS IS IRREVERSIBLE: YouTrack has no \
                        trash for issues, so the issue and its full history are lost with no \
                        way to recover them. Only call this with explicit user confirmation."
    )]
    async fn delete_issue(&self, params: Parameters<DeleteIssueRequest>) -> CallToolResult {
        let DeleteIssueRequest { issue_id } = params.0;

        match self.client.delete_issue(&issue_id).await {
            Ok(()) => tool_ok_result(&DeleteIssueResult {
                issue_id,
                deleted: true,
            }),
            Err(error) => tool_error_result(error.into()),
        }
    }

    // Intentionally not paginated: YouTrack's `issues/{id}/links` endpoint
    // returns the complete set of links in one response and has no
    // `$top`/`$skip` support, unlike the listing endpoints above. There is
    // no `has_more`/`next_cursor` to compute, so `IssueLinks` returns a
    // bare result by design, not by oversight.
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
    async fn create_issue_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "idReadable": "DEMO-1", "summary": "Fix the thing" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .create_issue(Parameters(CreateIssueRequest {
                project_id: "0-1".to_string(),
                summary: "Fix the thing".to_string(),
                description: None,
                fields: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["idReadable"], "DEMO-1");
    }

    #[tokio::test]
    async fn create_issue_with_an_unknown_custom_field_maps_to_a_tool_error_naming_it() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!([{
                    "canBeEmpty": true,
                    "field": { "name": "Priority" },
                }])),
            )
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let mut fields = std::collections::HashMap::new();
        fields.insert("Bogus".to_string(), serde_json::json!("whatever"));

        let result = tool_server
            .create_issue(Parameters(CreateIssueRequest {
                project_id: "0-1".to_string(),
                summary: "Fix the thing".to_string(),
                description: None,
                fields: Some(fields),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "unknown_custom_field");
        assert!(structured["message"].as_str().unwrap().contains("Bogus"));
    }

    #[tokio::test]
    async fn update_issue_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "idReadable": "DEMO-1", "summary": "New summary" }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .update_issue(Parameters(UpdateIssueRequest {
                issue_id: "DEMO-1".to_string(),
                summary: Some("New summary".to_string()),
                description: None,
                fields: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["summary"], "New summary");
    }

    #[tokio::test]
    async fn delete_issue_returns_a_successful_result_confirming_the_deleted_id() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-1"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .delete_issue(Parameters(DeleteIssueRequest {
                issue_id: "DEMO-1".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["issue_id"], "DEMO-1");
        assert_eq!(structured["deleted"], true);
    }

    #[tokio::test]
    async fn delete_issue_maps_404_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-404"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .delete_issue(Parameters(DeleteIssueRequest {
                issue_id: "DEMO-404".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "not_found");
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
