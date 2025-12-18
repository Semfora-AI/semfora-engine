//! Common test utilities and fixtures for semfora-engine integration tests
//!
//! This module provides:
//! - `TestRepo` builder for creating test repositories with various structures
//! - Custom assertions for validating CLI output and symbol extraction
//! - Helper functions for parsing TOON and JSON output

pub mod test_repo;
pub mod assertions;

pub use test_repo::TestRepo;
pub use assertions::*;
