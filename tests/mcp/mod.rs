//! MCP server tests module
//!
//! Tests for all 18 MCP tool handlers:
//! - Direct unit tests (handler_unit_tests.rs)
//! - JSON-RPC protocol tests (protocol_tests.rs)
//! - E2E workflow tests (workflow_tests.rs)
//! - Helper function tests (helpers_tests.rs)
//! - Formatting function tests (formatting_tests.rs)

pub mod formatting_tests;
pub mod handler_unit_tests;
pub mod helpers_tests;
pub mod protocol_tests;
pub mod workflow_tests;
