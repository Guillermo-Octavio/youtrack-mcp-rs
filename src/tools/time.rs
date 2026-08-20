//! Time-tracking-domain MCP tools: work items on issues.
//!
//! Every handler calls the real YouTrack REST API.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::ToolError;
use crate::render::DeleteWorkItemResult;

use super::{cursor_skip, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListWorkItemsRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Maximum number of work items to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct AddWorkItemRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Work item duration, in minutes.
    pub duration_minutes: u32,
    /// Optional description of the work performed.
    #[serde(default)]
    pub text: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeleteWorkItemRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// Work item id.
    pub work_item_id: String,
}

#[tool_router(router = time_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(
        description = "List the time-tracking work items logged on an issue. Supports pagination via `cursor`."
    )]
    async fn list_work_items(&self, params: Parameters<ListWorkItemsRequest>) -> CallToolResult {
        let ListWorkItemsRequest {
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
            .list_work_items(&issue_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "Log a time-tracking work item on an issue.")]
    async fn add_work_item(&self, params: Parameters<AddWorkItemRequest>) -> CallToolResult {
        let AddWorkItemRequest {
            issue_id,
            duration_minutes,
            text,
        } = params.0;

        if duration_minutes == 0 {
            return tool_error_result(ToolError {
                code: "validation",
                message: "work item duration must be greater than 0 minutes".to_string(),
                hint: Some("Pass a positive `duration_minutes`.".to_string()),
            });
        }

        match self
            .client
            .add_work_item(&issue_id, duration_minutes, text.as_deref())
            .await
        {
            Ok(work_item) => tool_ok_result(&work_item),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(
        description = "Permanently delete a time-tracking work item from an issue. This cannot be undone."
    )]
    async fn delete_work_item(&self, params: Parameters<DeleteWorkItemRequest>) -> CallToolResult {
        let DeleteWorkItemRequest {
            issue_id,
            work_item_id,
        } = params.0;

        match self.client.delete_work_item(&issue_id, &work_item_id).await {
            Ok(()) => tool_ok_result(&DeleteWorkItemResult {
                issue_id,
                work_item_id,
                deleted: true,
            }),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::time_router()
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
    async fn list_work_items_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "1", "date": 0, "duration": { "minutes": 30, "presentation": "30m" } }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_work_items(Parameters(ListWorkItemsRequest {
                issue_id: "DEMO-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["id"], "1");
    }

    #[tokio::test]
    async fn list_work_items_maps_an_invalid_cursor_to_a_validation_error() {
        let tool_server = test_server("https://example.myjetbrains.com");

        let result = tool_server
            .list_work_items(Parameters(ListWorkItemsRequest {
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
    async fn add_work_item_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!(
                { "id": "1", "duration": { "minutes": 30, "presentation": "30m" } }
            )))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .add_work_item(Parameters(AddWorkItemRequest {
                issue_id: "DEMO-1".to_string(),
                duration_minutes: 30,
                text: Some("Worked on it".to_string()),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["id"], "1");
    }

    #[tokio::test]
    async fn add_work_item_with_zero_duration_returns_a_validation_error_without_a_request() {
        let server = MockServer::start().await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .add_work_item(Parameters(AddWorkItemRequest {
                issue_id: "DEMO-1".to_string(),
                duration_minutes: 0,
                text: None,
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "validation");
        let requests = server.received_requests().await.unwrap();
        assert!(requests.is_empty());
    }

    #[tokio::test]
    async fn delete_work_item_returns_a_successful_result_with_rendered_text_and_json() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems/wi-1"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .delete_work_item(Parameters(DeleteWorkItemRequest {
                issue_id: "DEMO-1".to_string(),
                work_item_id: "wi-1".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["deleted"], true);
    }

    #[tokio::test]
    async fn delete_work_item_maps_404_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("DELETE"))
            .and(path("/api/issues/DEMO-1/timeTracking/workItems/missing"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .delete_work_item(Parameters(DeleteWorkItemRequest {
                issue_id: "DEMO-1".to_string(),
                work_item_id: "missing".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "not_found");
    }
}
