//! `youtrack-mcp-rs` library crate.
//!
//! Exposes shared infrastructure used across the MCP server, plus the
//! `tools` module wiring every YouTrack MCP tool into a single `rmcp`
//! server. Tool domain logic lands in a later phase of the SDD change
//! `rust-rewrite`; this phase only wires transport and discoverability.

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
