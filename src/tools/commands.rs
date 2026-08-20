//! YouTrack command-domain MCP tools: applying command-language actions to
//! one or many issues.
//!
//! Both `apply_command` and `bulk_command` share the same YouTrack REST
//! shape (`POST /commands` with a `query` and an `issues` array): a
//! single issue is just a one-element array. Both tool handlers below
//! call [`YouTrackClient::apply_command`](crate::client::YouTrackClient::apply_command),
//! which takes a slice of issue ids either way, so the dedup lives in
//! that one shared client method while each tool keeps its own distinct
//! request/response shape.

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::CallToolResult,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::Deserialize;

use crate::error::ToolError;
use crate::render::CommandResult;

use super::{tool_error_result, tool_ok_result, ToolServer};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ApplyCommandRequest {
    /// Issue id or readable id, e.g. `DEMO-1`.
    pub issue_id: String,
    /// YouTrack command text, e.g. `State Fixed`.
    pub command: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BulkCommandRequest {
    /// Issue ids or readable ids to apply the command to.
    pub issue_ids: Vec<String>,
    /// YouTrack command text, e.g. `State Fixed`.
    pub command: String,
}

#[tool_router(router = commands_router, vis = "pub(super)")]
impl ToolServer {
    #[tool(description = "Apply a YouTrack command to a single issue.")]
    async fn apply_command(&self, params: Parameters<ApplyCommandRequest>) -> CallToolResult {
        let ApplyCommandRequest { issue_id, command } = params.0;

        match self
            .client
            .apply_command(std::slice::from_ref(&issue_id), &command)
            .await
        {
            Ok(()) => tool_ok_result(&CommandResult {
                issue_ids: vec![issue_id],
                command,
            }),
            Err(error) => tool_error_result(error.into()),
        }
    }

    #[tool(description = "Apply a YouTrack command to multiple issues at once.")]
    async fn bulk_command(&self, params: Parameters<BulkCommandRequest>) -> CallToolResult {
        let BulkCommandRequest { issue_ids, command } = params.0;
        let issue_ids: Vec<String> = issue_ids
            .into_iter()
            .map(|id| id.trim().to_string())
            .filter(|id| !id.is_empty())
            .collect();

        if issue_ids.is_empty() {
            return tool_error_result(ToolError {
                code: "validation",
                message: "no issue ids were provided".to_string(),
                hint: Some("Pass at least one issue id in `issue_ids`.".to_string()),
            });
        }

        match self.client.apply_command(&issue_ids, &command).await {
            Ok(()) => tool_ok_result(&CommandResult { issue_ids, command }),
            Err(error) => tool_error_result(error.into()),
        }
    }
}

/// This domain's tool router, merged into [`ToolServer::new`].
pub(super) fn router() -> ToolRouter<ToolServer> {
    ToolServer::commands_router()
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
    async fn apply_command_to_one_issue_legacy_apply_command_shape() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .apply_command(Parameters(ApplyCommandRequest {
                issue_id: "DEMO-1".to_string(),
                command: "State Fixed".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["issue_ids"], serde_json::json!(["DEMO-1"]));
        assert_eq!(structured["command"], "State Fixed");
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(
            body["issues"],
            serde_json::json!([{ "idReadable": "DEMO-1" }])
        );
    }

    #[tokio::test]
    async fn apply_command_maps_401_to_a_tool_error() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .apply_command(Parameters(ApplyCommandRequest {
                issue_id: "DEMO-1".to_string(),
                command: "State Fixed".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "unauthorized");
    }

    #[tokio::test]
    async fn apply_command_to_multiple_issues_legacy_bulk_command_shape() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .bulk_command(Parameters(BulkCommandRequest {
                issue_ids: vec!["DEMO-1".to_string(), "DEMO-2".to_string()],
                command: "State Fixed".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(
            structured["issue_ids"],
            serde_json::json!(["DEMO-1", "DEMO-2"])
        );
        let requests = server.received_requests().await.unwrap();
        let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
        assert_eq!(
            body["issues"],
            serde_json::json!([
                { "idReadable": "DEMO-1" },
                { "idReadable": "DEMO-2" },
            ])
        );
    }

    #[tokio::test]
    async fn bulk_command_trims_and_drops_blank_issue_ids() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/commands"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .bulk_command(Parameters(BulkCommandRequest {
                issue_ids: vec![" DEMO-1 ".to_string(), "".to_string(), "  ".to_string()],
                command: "State Fixed".to_string(),
            }))
            .await;

        assert_ne!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["issue_ids"], serde_json::json!(["DEMO-1"]));
    }

    #[tokio::test]
    async fn bulk_command_with_no_issue_ids_returns_a_validation_error_without_a_request() {
        let server = MockServer::start().await;
        let tool_server = test_server(&server.uri());

        let result = tool_server
            .bulk_command(Parameters(BulkCommandRequest {
                issue_ids: vec!["  ".to_string()],
                command: "State Fixed".to_string(),
            }))
            .await;

        assert_eq!(result.is_error, Some(true));
        let structured = result.structured_content.unwrap();
        assert_eq!(structured["code"], "validation");
        let requests = server.received_requests().await.unwrap();
        assert!(requests.is_empty());
    }
}
