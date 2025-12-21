//! Shared indexing module for CLI and MCP
//!
//! This module provides unified file collection and parallel index generation,
//! ensuring consistent behavior between CLI commands and MCP server handlers.
//!
//! # Key Features
//!
//! - **Parallel Processing**: Uses Rayon for multi-threaded file analysis
//! - **Progress Reporting**: Optional callback for progress updates
//! - **Error Handling**: Collects errors without stopping the entire operation
//!
//! # Example
//!
//! ```ignore
//! use semfora_engine::indexing::{collect_files, analyze_files_parallel};
//!
//! let files = collect_files(&repo_dir, 10, &[]);
//! let result = analyze_files_parallel(&files, None, false);
//!
//! println!("Analyzed {} files, {} errors", result.summaries.len(), result.errors);
//! ```

mod files;
mod generation;

pub use files::{collect_files, collect_files_recursive, should_skip_path};
pub use generation::{
    analyze_files_parallel, analyze_files_with_stats, IndexGenerationResult,
    IndexingProgressCallback,
};
