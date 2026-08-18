//! End-to-end harness placeholder.
//!
//! This test proves that the `cargo test` harness compiles and runs
//! end-to-end for this crate. It will grow into a real stdio MCP
//! transport test in a later phase; for now it only asserts the
//! server identity constant exported by the library crate.

use youtrack_mcp_rs::SERVER_NAME;

#[test]
fn server_name_matches_binary_name() {
    assert_eq!(SERVER_NAME, "youtrack-mcp");
}
