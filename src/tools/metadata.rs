//! Project-metadata-domain MCP tools: projects, custom fields, tags, and
//! saved searches.
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
pub struct ListProjectsRequest {
    /// Maximum number of projects to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListProjectFieldsRequest {
    /// Short name or id of the project.
    pub project_id: String,
    /// Maximum number of custom fields to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListTagsRequest {
    /// Maximum number of tags to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSavedSearchesRequest {
    /// Maximum number of saved searches to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[tool_router(router = metadata_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(
        description = "List all YouTrack projects visible to the current user. Supports pagination via `cursor`."
    )]
    async fn list_projects(&self, params: Parameters<ListProjectsRequest>) -> CallToolResult {
        let ListProjectsRequest { limit, cursor } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_projects(limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "List the custom fields configured on a project. Supports pagination via `cursor`."
    )]
    async fn list_project_fields(
        &self,
        params: Parameters<ListProjectFieldsRequest>,
    ) -> CallToolResult {
        let ListProjectFieldsRequest {
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
            .list_project_fields(&project_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "List all issue tags visible to the current user. Supports pagination via `cursor`."
    )]
    async fn list_tags(&self, params: Parameters<ListTagsRequest>) -> CallToolResult {
        let ListTagsRequest { limit, cursor } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_tags(limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "List all saved searches visible to the current user. Supports pagination via `cursor`."
    )]
    async fn list_saved_searches(
        &self,
        params: Parameters<ListSavedSearchesRequest>,
    ) -> CallToolResult {
        let ListSavedSearchesRequest { limit, cursor } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_saved_searches(limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::metadata_router()
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
    async fn list_projects_returns_a_page_of_projects() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "0-1", "name": "Demo", "shortName": "DEMO" }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_projects(Parameters(ListProjectsRequest {
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["shortName"], "DEMO");
    }

    #[tokio::test]
    async fn list_project_fields_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .list_project_fields(Parameters(ListProjectFieldsRequest {
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
    async fn list_project_fields_returns_configured_custom_fields() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/admin/projects/0-1/customFields"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "canBeEmpty": true, "field": { "name": "Priority" } }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_project_fields(Parameters(ListProjectFieldsRequest {
                project_id: "0-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["field"]["name"], "Priority");
    }

    #[tokio::test]
    async fn list_tags_returns_a_page_of_tags() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/tags"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "1", "name": "urgent" }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_tags(Parameters(ListTagsRequest {
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["name"], "urgent");
    }

    #[tokio::test]
    async fn list_tags_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .list_tags(Parameters(ListTagsRequest {
                limit: None,
                cursor: Some("not-a-valid-cursor".to_string()),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "validation");
    }

    #[tokio::test]
    async fn list_saved_searches_returns_a_page_with_owner_and_query() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/savedQueries"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                {
                    "id": "1",
                    "name": "My bugs",
                    "query": "assignee: me #Bug",
                    "owner": { "login": "jdoe", "fullName": "Jane Doe" },
                }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_saved_searches(Parameters(ListSavedSearchesRequest {
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["name"], "My bugs");
    }

    #[tokio::test]
    async fn list_saved_searches_maps_401_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/savedQueries"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_saved_searches(Parameters(ListSavedSearchesRequest {
                limit: None,
                cursor: None,
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "unauthorized");
    }
}
