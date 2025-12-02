//! Request and response types for the MCP server
//!
//! This module contains all the request/response structs used by the MCP tools.

use rmcp::schemars;
use serde::Deserialize;

// ============================================================================
// Analysis Request Types
// ============================================================================

/// Request to analyze a single file
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AnalyzeFileRequest {
    /// The absolute or relative path to the file to analyze
    #[schemars(description = "Path to the source file to analyze")]
    pub path: String,

    /// Output format: "toon" (default) or "json"
    #[schemars(description = "Output format: 'toon' (compact) or 'json' (structured)")]
    pub format: Option<String>,
}

/// Request to analyze a directory
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AnalyzeDirectoryRequest {
    /// The path to the directory to analyze
    #[schemars(description = "Path to the directory to analyze")]
    pub path: String,

    /// Maximum depth for recursive analysis (default: 10)
    #[schemars(description = "Maximum directory depth to traverse (default: 10)")]
    pub max_depth: Option<usize>,

    /// Whether to include only the summary overview
    #[schemars(description = "If true, only return the repository overview, not individual files")]
    pub summary_only: Option<bool>,

    /// File extensions to include (e.g., ["ts", "tsx", "js"])
    #[schemars(
        description = "File extensions to include (e.g., ['ts', 'tsx']). If empty, all supported extensions are included."
    )]
    pub extensions: Option<Vec<String>>,
}

/// Request to analyze git diff
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AnalyzeDiffRequest {
    /// The base branch or commit to compare against (e.g., "main", "HEAD~1").
    /// Use "HEAD" with target_ref "WORKING" to see uncommitted changes.
    #[schemars(description = "Base branch or commit to compare against (e.g., 'main', 'HEAD~1'). Use 'HEAD' with target_ref='WORKING' for uncommitted changes.")]
    pub base_ref: String,

    /// The target branch or commit. Use "WORKING" to compare against uncommitted changes in the working tree.
    /// Defaults to "HEAD" for committed changes.
    #[schemars(description = "Target branch or commit (defaults to 'HEAD'). Use 'WORKING' to analyze uncommitted changes vs base_ref.")]
    pub target_ref: Option<String>,

    /// Working directory (defaults to current directory)
    #[schemars(description = "Working directory for git operations (defaults to current directory)")]
    pub working_dir: Option<String>,
}

/// Request to list supported languages
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListLanguagesRequest {}

// ============================================================================
// Sharded Index Request Types
// ============================================================================

/// Request to get repository overview from sharded index
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetRepoOverviewRequest {
    /// Path to the repository (defaults to current directory)
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,
}

/// Request to get a module from sharded index
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetModuleRequest {
    /// Path to the repository (defaults to current directory)
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,

    /// Name of the module to retrieve (e.g., "api", "components", "lib")
    #[schemars(description = "Module name (e.g., 'api', 'components', 'lib', 'tests')")]
    pub module_name: String,
}

/// Request to get a symbol from sharded index
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolRequest {
    /// Path to the repository (defaults to current directory)
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,

    /// Symbol hash (from repo_overview or module listing)
    #[schemars(description = "Symbol hash from the repo overview or module shard")]
    pub symbol_hash: String,
}

/// Request to list modules in a sharded index
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListModulesRequest {
    /// Path to the repository (defaults to current directory)
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,
}

/// Request to generate/regenerate sharded index
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GenerateIndexRequest {
    /// Path to the repository to index
    #[schemars(description = "Path to the repository to index")]
    pub path: String,

    /// Maximum directory depth (default: 10)
    #[schemars(description = "Maximum directory depth for file collection (default: 10)")]
    pub max_depth: Option<usize>,
}

/// Request to get call graph from sharded index
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCallGraphRequest {
    /// Path to the repository (defaults to current directory)
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,
}

/// Request to get source code for a symbol (surgical read)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSymbolSourceRequest {
    /// Path to the source file
    #[schemars(description = "Path to the source file containing the symbol")]
    pub file_path: String,

    /// Start line (1-indexed). If provided with end_line, reads that range.
    #[schemars(description = "Start line number (1-indexed)")]
    pub start_line: Option<usize>,

    /// End line (1-indexed, inclusive)
    #[schemars(description = "End line number (1-indexed, inclusive)")]
    pub end_line: Option<usize>,

    /// Symbol hash to look up (alternative to line numbers)
    #[schemars(description = "Symbol hash from the index - will look up line range automatically")]
    pub symbol_hash: Option<String>,

    /// Context lines to include before/after the symbol (default: 5)
    #[schemars(description = "Number of context lines before and after the symbol (default: 5)")]
    pub context: Option<usize>,
}

// ============================================================================
// Query-Driven API Types
// ============================================================================

/// Search for symbols by name across the repository (lightweight, query-driven)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSymbolsRequest {
    /// Search query - matches symbol names (case-insensitive, partial match)
    #[schemars(description = "Search query - matches symbol names (case-insensitive, partial match)")]
    pub query: String,

    /// Optional: filter by module name
    #[schemars(description = "Filter results to a specific module")]
    pub module: Option<String>,

    /// Optional: filter by symbol kind (fn, struct, component, enum, etc.)
    #[schemars(description = "Filter by symbol kind (fn, struct, component, enum, trait, etc.)")]
    pub kind: Option<String>,

    /// Optional: filter by risk level (high, medium, low)
    #[schemars(description = "Filter by risk level (high, medium, low)")]
    pub risk: Option<String>,

    /// Maximum results to return (default: 20, max: 100)
    #[schemars(description = "Maximum results to return (default: 20, max: 100)")]
    pub limit: Option<usize>,

    /// Repository path (defaults to current directory)
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,
}

/// List all symbols in a specific module (lightweight index only)
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSymbolsRequest {
    /// Module name to list symbols from
    #[schemars(description = "Module name to list symbols from (e.g., 'api', 'components')")]
    pub module: String,

    /// Optional: filter by symbol kind
    #[schemars(description = "Filter by symbol kind (fn, struct, component, enum, trait, etc.)")]
    pub kind: Option<String>,

    /// Optional: filter by risk level
    #[schemars(description = "Filter by risk level (high, medium, low)")]
    pub risk: Option<String>,

    /// Maximum results (default: 50, max: 200)
    #[schemars(description = "Maximum results to return (default: 50, max: 200)")]
    pub limit: Option<usize>,

    /// Repository path
    #[schemars(description = "Path to the repository root (defaults to current directory)")]
    pub path: Option<String>,
}

// ============================================================================
// Re-exports
// ============================================================================

// SymbolIndexEntry is defined in cache.rs and re-exported from lib.rs
