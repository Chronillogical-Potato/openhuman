//! Unit tests for the MCP clients RPC handlers.
//!
//! What is testable without a running service is the blank-identifier guard.
//! The operations themselves are covered in `tinymcp`; what this layer adds is
//! the RPC shape, which the end-to-end suites exercise against a live process.

use super::*;

#[test]
fn a_blank_identifier_is_refused_with_the_field_name() {
    // The frontend surfaces this text, so it has to name what was missing.
    let error = require("   ", "server_id").expect_err("a blank identifier");
    assert_eq!(error, "server_id must not be empty");
}

#[test]
fn an_identifier_is_trimmed_before_it_is_used() {
    assert_eq!(require("  srv-1  ", "server_id").unwrap(), "srv-1");
}
