//! `youtrack-mcp-rs` library crate.
//!
//! Exposes shared infrastructure used across the MCP server, plus the
//! `tools` module wiring every YouTrack MCP tool into a single `rmcp`
//! server. Every one of the 35 registered tools calls the real YouTrack
//! REST API; see `youtrack_mcp_rs::tools` for the domain-by-domain
//! breakdown and `README.md` for the full tool list grouped by area.

pub mod client;
pub mod config;
pub mod error;
pub mod payload;
pub mod render;
pub mod tools;
pub mod youtrack;

/// Canonical name of the MCP server binary, used for identification
/// in MCP `initialize` responses and end-to-end tests.
pub const SERVER_NAME: &str = "youtrack-mcp";
