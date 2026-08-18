//! End-to-end MCP transport test for the YouTrack MCP server.
//!
//! Drives the real [`ToolServer`] over an in-process duplex byte stream —
//! the exact transport shape `rmcp::transport::stdio()` uses in
//! production, minus touching the process's real stdin/stdout — with a
//! real `rmcp` client performing the MCP `initialize` handshake and a
//! `tools/list` call. This proves the server is reachable over the MCP
//! protocol and exposes exactly the 34 tools this phase registers.

use std::sync::Arc;

use rmcp::{model::ClientInfo, ServiceExt};
use url::Url;
use youtrack_mcp_rs::client::YouTrackClient;
use youtrack_mcp_rs::config::Config;
use youtrack_mcp_rs::tools::ToolServer;

/// Builds a [`ToolServer`] with a client pointed at a placeholder URL.
/// Neither test below exercises a real YouTrack REST call, so this never
/// needs to reach a real (or mock) server.
fn test_server() -> ToolServer {
    let config = Config {
        youtrack_url: Url::parse("https://example.myjetbrains.com").unwrap(),
        youtrack_token: "test-token".to_string(),
    };
    ToolServer::new(Arc::new(YouTrackClient::new(&config).unwrap()))
}

const EXPECTED_TOOL_NAMES: &[&str] = &[
    "search_issues",
    "get_issue",
    "list_projects",
    "list_project_fields",
    "list_links",
    "list_agile_boards",
    "list_sprints",
    "get_sprint",
    "create_issue",
    "update_issue",
    "delete_issue",
    "list_articles",
    "get_article",
    "create_article",
    "update_article",
    "delete_article",
    "list_article_attachments",
    "upload_article_attachment",
    "apply_command",
    "bulk_command",
    "list_comments",
    "add_comment",
    "list_attachments",
    "upload_attachment",
    "download_attachment",
    "get_current_user",
    "search_users",
    "list_work_items",
    "add_work_item",
    "delete_work_item",
    "get_issue_activity",
    "list_tags",
    "list_saved_searches",
    "list_watchers",
];

#[tokio::test]
async fn stdio_transport_exposes_exactly_the_34_registered_tools() {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_task = tokio::spawn(async move {
        test_server()
            .serve(server_transport)
            .await
            .expect("server should accept the in-process transport")
            .waiting()
            .await
            .expect("server should shut down cleanly");
    });

    let client = ClientInfo::default()
        .serve(client_transport)
        .await
        .expect("client should complete the MCP initialize handshake");

    let tools = client
        .list_tools(None)
        .await
        .expect("tools/list should succeed");

    let mut names: Vec<&str> = tools.tools.iter().map(|tool| tool.name.as_ref()).collect();
    names.sort_unstable();

    let mut expected: Vec<&str> = EXPECTED_TOOL_NAMES.to_vec();
    expected.sort_unstable();

    assert_eq!(
        names, expected,
        "stdio transport must expose exactly the 34 YouTrack MCP tools, no more and no fewer"
    );

    client
        .cancel()
        .await
        .expect("client should cancel the session cleanly");
    server_task.await.expect("server task should not panic");
}

#[tokio::test]
async fn calling_a_stub_tool_returns_a_not_implemented_error() {
    let (server_transport, client_transport) = tokio::io::duplex(16 * 1024);

    let server_task = tokio::spawn(async move {
        test_server()
            .serve(server_transport)
            .await
            .expect("server should accept the in-process transport")
            .waiting()
            .await
            .expect("server should shut down cleanly");
    });

    let client = ClientInfo::default()
        .serve(client_transport)
        .await
        .expect("client should complete the MCP initialize handshake");

    let arguments = serde_json::json!({ "project_id": "DEMO", "summary": "New issue" })
        .as_object()
        .unwrap()
        .clone();
    let result = client
        .call_tool(
            rmcp::model::CallToolRequestParams::new("create_issue").with_arguments(arguments),
        )
        .await
        .expect("call_tool should return a tool-level result, not a protocol error");

    assert_eq!(
        result.is_error,
        Some(true),
        "an unimplemented stub tool must report a tool-level error"
    );
    let structured = result
        .structured_content
        .expect("stub result must carry structured error content");
    assert_eq!(structured["code"], "not_implemented");
    assert!(
        structured["message"]
            .as_str()
            .unwrap()
            .contains("create_issue"),
        "the error message must name the specific tool that is not implemented"
    );

    client
        .cancel()
        .await
        .expect("client should cancel the session cleanly");
    server_task.await.expect("server task should not panic");
}
