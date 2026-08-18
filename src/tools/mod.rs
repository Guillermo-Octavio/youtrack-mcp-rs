//! MCP tool registrations for the YouTrack MCP server.
//!
//! [`ToolServer`] is the single `rmcp` server type for this crate. Each
//! YouTrack domain owns its own submodule (see below) that declares an
//! `impl ToolServer` block annotated with `#[rmcp::tool_router]`;
//! [`ToolServer::new`] combines every domain's router into one dispatch
//! table with [`ToolRouter::merge`]/`+`.
//!
//! Every tool handler registered here is a Phase 2 stub: it accepts a
//! minimal typed input, does no real work, and always returns
//! [`stub_result`]. Real YouTrack REST calls land in a later phase of the
//! `rust-rewrite` change.

mod activity;
mod agile;
mod articles;
mod attachments;
mod commands;
mod comments;
mod issues;
mod metadata;
mod time;
mod users;

use rmcp::{
    handler::server::router::tool::ToolRouter,
    model::{CallToolResult, ContentBlock},
    tool_handler, ServerHandler,
};

use crate::{error::ToolError, payload::ToolPayload};

/// Aggregate MCP server exposing every YouTrack MCP tool.
#[derive(Debug, Clone)]
pub struct ToolServer {
    tool_router: ToolRouter<Self>,
}

impl ToolServer {
    /// Builds the server with every domain's tools registered.
    pub fn new() -> Self {
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
        }
    }
}

impl Default for ToolServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_handler(
    router = self.tool_router,
    name = "youtrack-mcp",
    instructions = "YouTrack MCP server. Every tool in this Phase 2 build is a stub \
                     that returns a `not_implemented` error; domain logic lands in a \
                     later phase of the rust-rewrite change."
)]
impl ServerHandler for ToolServer {}

/// Builds the standard "not yet implemented" [`CallToolResult`] every stub
/// tool handler in this module returns, reusing [`ToolError`] (Phase 1
/// shared error shape) and [`ToolPayload`] (Phase 1 text/JSON pairing) so
/// every stub tool reports failures the same actionable way real tools
/// will in later phases.
pub(crate) fn stub_result(tool_name: &'static str) -> CallToolResult {
    let error = ToolError {
        code: "not_implemented",
        message: format!("`{tool_name}` is not implemented yet."),
        hint: Some(
            "Domain logic for this tool lands in a later phase of the rust-rewrite change."
                .to_string(),
        ),
    };
    let payload = ToolPayload::from_value(&error).expect("ToolError always serializes to JSON");

    let mut result = CallToolResult::error(vec![ContentBlock::text(payload.text)]);
    result.structured_content = Some(payload.json);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_server_registers_exactly_34_tools() {
        let server = ToolServer::new();

        assert_eq!(server.tool_router.list_all().len(), 34);
    }

    #[test]
    fn stub_result_names_the_specific_tool_and_marks_the_call_as_an_error() {
        let result = stub_result("get_issue");

        assert_eq!(result.is_error, Some(true));
        let structured = result
            .structured_content
            .expect("stub result is structured");
        assert_eq!(structured["code"], "not_implemented");
        assert_eq!(structured["message"], "`get_issue` is not implemented yet.");
    }

    #[test]
    fn stub_result_message_changes_with_the_tool_name() {
        let result = stub_result("list_projects");

        let structured = result
            .structured_content
            .expect("stub result is structured");
        assert_eq!(
            structured["message"],
            "`list_projects` is not implemented yet."
        );
    }
}
