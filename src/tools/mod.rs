//! MCP tool registrations for the YouTrack MCP server.
//!
//! [`ToolServer`] is the single `rmcp` server type for this crate. Each
//! YouTrack domain owns its own submodule (see below) that declares an
//! `impl ToolServer` block annotated with `#[rmcp::tool_router]`;
//! [`ToolServer::new`] combines every domain's router into one dispatch
//! table with [`ToolRouter::merge`]/`+`.
//!
//! Every tool handler registered here calls the real YouTrack REST API.

mod activity;
mod agile;
mod articles;
mod attachments;
mod commands;
mod comments;
mod issues;
mod local_file;
mod metadata;
mod time;
mod users;

use std::sync::Arc;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, ContentBlock},
    tool_handler, ServerHandler,
};
use serde::Serialize;

use crate::{
    client::page::Cursor, client::YouTrackClient, error::ToolError, payload::Render,
    payload::ToolPayload,
};

/// Default page size for a paginated list tool when the caller does not
/// pass an explicit `limit`.
pub(crate) const DEFAULT_LIMIT: usize = 50;

/// Aggregate MCP server exposing every YouTrack MCP tool.
#[derive(Debug, Clone)]
pub struct ToolServer {
    tool_router: ToolRouter<Self>,
    client: Arc<YouTrackClient>,
}

impl ToolServer {
    /// Builds the server with every domain's tools registered, sharing
    /// one [`YouTrackClient`] across every domain's REST calls.
    pub fn new(client: Arc<YouTrackClient>) -> Self {
        Self {
            tool_router: activity::router()
                + agile::router()
                + articles::router()
                + attachments::router()
                + comments::router()
                + commands::router()
                + issues::router()
                + metadata::router()
                + time::router()
                + users::router(),
            client,
        }
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "youtrack-mcp",
    instructions = "YouTrack MCP server. Exposes 34 tools covering issue search and \
                     CRUD, agile boards, the knowledge base, workflow commands, \
                     comments, attachments, users, time tracking, and issue history. \
                     Every tool calls the real YouTrack REST API configured via \
                     YOUTRACK_URL and YOUTRACK_TOKEN."
)]
impl ServerHandler for ToolServer {}

/// Builds a successful [`CallToolResult`] pairing `value`'s rendered text
/// with its structured JSON form, for a tool that completed a real
/// YouTrack REST call.
pub(crate) fn tool_ok_result<T: Serialize + Render>(value: &T) -> CallToolResult {
    let payload = ToolPayload::from_value(value).expect("tool result always serializes to JSON");
    let mut result = CallToolResult::success(vec![ContentBlock::text(payload.text)]);
    result.structured_content = Some(payload.json);
    result
}

/// Builds an error [`CallToolResult`] from a [`ToolError`], the shape
/// every real tool handler returns when a YouTrack REST call fails.
pub(crate) fn tool_error_result(error: ToolError) -> CallToolResult {
    let payload = ToolPayload::from_value(&error).expect("ToolError always serializes to JSON");
    let mut result = CallToolResult::error(vec![ContentBlock::text(payload.text)]);
    result.structured_content = Some(payload.json);
    result
}

/// Decodes an optional pagination cursor into the `skip` offset it
/// encodes, defaulting to `0` when no cursor was passed. Returns a
/// [`ToolError`] when `cursor` does not decode to a valid [`Cursor`].
pub(crate) fn cursor_skip(cursor: Option<&str>) -> Result<usize, ToolError> {
    match cursor {
        None => Ok(0),
        Some(raw) => Cursor::decode(raw).map(|cursor| cursor.skip).map_err(|_| ToolError {
            code: "validation",
            message: "invalid pagination cursor".to_string(),
            hint: Some(
                "Pass the exact cursor a previous call returned, or omit it to start from the first page."
                    .to_string(),
            ),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use url::Url;

    pub(crate) fn test_client() -> Arc<YouTrackClient> {
        let config = Config {
            youtrack_url: Url::parse("https://example.myjetbrains.com").unwrap(),
            youtrack_token: "test-token".to_string(),
        };
        Arc::new(YouTrackClient::new(&config).unwrap())
    }

    #[test]
    fn tool_server_registers_exactly_34_tools() {
        let server = ToolServer::new(test_client());

        assert_eq!(server.tool_router.list_all().len(), 34);
    }
}
