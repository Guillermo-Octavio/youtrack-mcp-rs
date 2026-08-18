//! `youtrack-mcp-rs` library crate.
//!
//! Currently exposes only the server identity constant used by the
//! binary and by tests. Domain logic (tools, transport, YouTrack
//! client) lands in later phases of the SDD change `rust-rewrite`.

/// Canonical name of the MCP server binary, used for identification
/// in MCP `initialize` responses and end-to-end tests.
pub const SERVER_NAME: &str = "youtrack-mcp";
