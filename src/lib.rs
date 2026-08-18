//! `youtrack-mcp-rs` library crate.
//!
//! Exposes shared infrastructure used across the MCP server. Domain
//! logic (tools, transport) lands in later phases of the SDD change
//! `rust-rewrite`.

pub mod error;
pub mod payload;

/// Canonical name of the MCP server binary, used for identification
/// in MCP `initialize` responses and end-to-end tests.
pub const SERVER_NAME: &str = "youtrack-mcp";
