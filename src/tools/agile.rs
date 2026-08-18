//! Agile-board-domain MCP tools: boards and sprints. Every handler calls
//! the real YouTrack REST API.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use super::{cursor_skip, tool_error_result, tool_ok_result, ToolServer, DEFAULT_LIMIT};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListAgileBoardsRequest {
    /// Maximum number of boards to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSprintsRequest {
    /// Agile board id.
    pub board_id: String,
    /// Maximum number of sprints to return (default 50).
    #[serde(default)]
    pub limit: Option<usize>,
    /// Opaque cursor from a previous call's response, to fetch the next
    /// page.
    #[serde(default)]
    pub cursor: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct GetSprintRequest {
    /// Agile board id the sprint belongs to.
    pub board_id: String,
    /// Sprint id.
    pub sprint_id: String,
}

#[tool_router(router = agile_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "List all agile boards. Supports pagination via `cursor`.")]
    async fn list_agile_boards(
        &self,
        params: Parameters<ListAgileBoardsRequest>,
    ) -> CallToolResult {
        let ListAgileBoardsRequest { limit, cursor } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_agile_boards(limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "List the sprints on an agile board. Supports pagination via `cursor`.")]
    async fn list_sprints(&self, params: Parameters<ListSprintsRequest>) -> CallToolResult {
        let ListSprintsRequest {
            board_id,
            limit,
            cursor,
        } = params.0;
        let skip = match cursor_skip(cursor.as_deref()) {
            Ok(skip) => skip,
            Err(error) => return tool_error_result(error),
        };

        match self
            .client
            .list_sprints(&board_id, limit.unwrap_or(DEFAULT_LIMIT), skip)
            .await
        {
            Ok(page) => tool_ok_result(&page),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "Get a single sprint by id, with its issues.")]
    async fn get_sprint(&self, params: Parameters<GetSprintRequest>) -> CallToolResult {
        let GetSprintRequest {
            board_id,
            sprint_id,
        } = params.0;

        match self.client.get_sprint(&board_id, &sprint_id).await {
            Ok(sprint) => tool_ok_result(&sprint),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::agile_router()
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
    async fn list_agile_boards_returns_a_page_of_boards() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/agiles"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([
                { "id": "1", "name": "Board 1" }
            ])))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_agile_boards(Parameters(ListAgileBoardsRequest {
                limit: None,
                cursor: None,
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["items"][0]["name"], "Board 1");
    }

    #[tokio::test]
    async fn list_sprints_maps_forbidden_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/agiles/board-1/sprints"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .list_sprints(Parameters(ListSprintsRequest {
                board_id: "board-1".to_string(),
                limit: None,
                cursor: None,
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "forbidden");
    }

    #[tokio::test]
    async fn get_sprint_returns_the_sprint_detail_with_its_issues() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/agiles/board-1/sprints/sprint-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": "sprint-1",
                "name": "Sprint 1",
                "issues": [{ "idReadable": "DEMO-1", "summary": "Fix the thing" }],
            })))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .get_sprint(Parameters(GetSprintRequest {
                board_id: "board-1".to_string(),
                sprint_id: "sprint-1".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["issues"][0]["idReadable"], "DEMO-1");
    }
}
