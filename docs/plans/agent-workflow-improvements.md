# Agent Workflow Improvements Plan

## Overview

This plan outlines improvements to the semfora-engine MCP server to optimize the agent workflow from first prompt to useful results, minimizing context usage while maximizing actionable information.

## Design Principles

1. **Semfora is read-only** - indexing and analysis only, no code editing
2. **Token efficiency** - minimize context consumption at every step
3. **Progressive disclosure** - lightweight context first, details on demand
4. **Quality validation** - leverage existing complexity/duplicate detection
5. **No curated lists** - use algorithmic approaches (BM25) over maintained word lists

---

## Phase 1: Quick Context Tool

### New Tool: `get_context`

**Purpose**: Provide immediate git and project context without full index dump.

**Target token cost**: ~200 tokens

**Response structure**:
```yaml
_type: context
repo_name: "semfora-engine"
branch: "main"
remote: "github.com/Semfora-AI/semfora-engine"
last_commit:
  hash: "04aa817"
  message: "feat: parallelize analysis and indexing"
  author: "..."
  date: "2025-12-14"
index_status: "fresh" | "stale"
stale_files: 3  # if stale
project_type: "Rust CLI + Library"
entry_points: ["src/main.rs", "src/lib.rs"]
```

**Implementation**:
- Git info: `git rev-parse`, `git remote`, `git log -1`
- Index status: Compare index timestamp vs file mtimes
- Project type: Already detected in RepoOverview
- Entry points: Already in RepoOverview

**Files to modify**:
- `src/mcp_server/mod.rs` - Add new tool handler
- `src/mcp_server/types.rs` - Add request/response types

---

## Phase 2: Lightweight Repo Summary

### Modified Tool: `get_repo_summary` (or flag on `get_repo_overview`)

**Purpose**: Replace overwhelming 5000+ token overview with focused 500 token summary.

**Changes**:
1. **Auto-exclude test directories** from module listing
   - Skip paths containing `/test-repos/`, `/tests/`, `/__tests__/`
   - Make configurable via `exclude_patterns` parameter

2. **Limit modules to top N by relevance**
   - Default: 20 modules
   - Sort by: symbol count, risk level, or entry point proximity
   - Parameter: `max_modules: Option<usize>`

3. **Include git context** (from Phase 1)

**Response structure**:
```yaml
_type: repo_summary
context:
  repo: "semfora-engine"
  branch: "main"
  last_commit: "04aa817"
framework: "Rust (bin+lib)"
patterns: ["CLI application", "MCP server", "AST analysis"]
modules[20]:  # Top 20 only
  - name: "mcp_server"
    purpose: "MCP protocol handlers"
    files: 3
    symbols: 57
    risk: "low"
  ...
stats:
  total_files: 136
  total_modules: 47  # Actual count (excluding test-repos)
  total_symbols: 1971
entry_points: ["src/main.rs"]
```

**Files to modify**:
- `src/shard.rs` - Add filtering logic to `generate_sharded_index`
- `src/mcp_server/mod.rs` - Add parameters, include git context
- `src/mcp_server/types.rs` - Extend request type

---

## Phase 3: BM25 Semantic Search

### New Tool: `semantic_search`

**Purpose**: Enable loose term queries like "authentication" or "error handling" that find conceptually related code, not just exact symbol name matches.

**Approach**: BM25 (Best Match 25) ranking algorithm
- No curated word lists required
- Indexes terms from: symbol names, comments, string literals, file paths
- Handles partial matches, stemming optional
- Fast: O(query_terms * matching_docs)

**Implementation options**:

#### Option A: Build BM25 index at shard time
- During `generate_index`, extract terms from each symbol
- Store inverted index in `bm25_index.json` alongside other shards
- Query-time: Load index, compute BM25 scores

#### Option B: Use existing `raw_search` with ranking
- Ripgrep already searches content
- Add BM25 scoring layer on top of grep results
- Less accurate but zero index overhead

**Recommended**: Option A for accuracy

**Request**:
```rust
pub struct SemanticSearchRequest {
    pub query: String,           // "authentication", "error handling"
    pub path: Option<String>,
    pub limit: Option<usize>,    // Default 20
    pub include_source: Option<bool>,
}
```

**Response structure**:
```yaml
_type: semantic_search_results
query: "authentication"
results[20]:
  - symbol: "validate_token"
    file: "src/auth/validate.rs"
    lines: "45-78"
    score: 0.89
    snippet: "/// Validates JWT token..."  # If include_source
    context_terms: ["token", "jwt", "session"]
  ...
related_queries: ["login", "session", "token", "credentials"]
```

**Files to create/modify**:
- `src/bm25.rs` (NEW) - BM25 implementation
- `src/shard.rs` - Add BM25 index generation
- `src/mcp_server/mod.rs` - Add tool handler
- `src/mcp_server/types.rs` - Add request/response types

**BM25 Implementation sketch**:
```rust
pub struct Bm25Index {
    // term -> [(doc_id, term_freq)]
    inverted_index: HashMap<String, Vec<(String, u32)>>,
    // doc_id -> doc_length
    doc_lengths: HashMap<String, u32>,
    avg_doc_length: f64,
    total_docs: u32,
}

impl Bm25Index {
    pub fn search(&self, query: &str, k: usize) -> Vec<(String, f64)> {
        // Standard BM25 with k1=1.2, b=0.75
    }
}
```

---

## Phase 4: Code Quality Validation Tool

### New Tool: `validate_symbol`

**Purpose**: Post-analysis quality check for a symbol. Useful for agents to verify code they're reviewing or after the user has made changes.

**Leverages existing functionality**:
- `calculate_complexity()` in `src/detectors/generic.rs:769`
- `find_duplicates` (already fast on massive codebases)
- `get_callers` for impact analysis

**Request**:
```rust
pub struct ValidateSymbolRequest {
    pub symbol_hash: Option<String>,  // Lookup by hash
    pub file_path: Option<String>,    // Or by file + line
    pub line: Option<usize>,
    pub path: Option<String>,         // Repo path
}
```

**Response structure**:
```yaml
_type: validation_result
symbol: "handle_request"
file: "src/server/handler.rs"
lines: "45-120"

complexity:
  cognitive: 12
  cyclomatic: 8
  max_nesting: 4
  risk: "medium"

duplicates:  # From find_duplicates, filtered to this symbol
  - symbol: "process_request"
    file: "src/api/processor.rs"
    similarity: 0.87

callers:  # Impact radius
  direct: 5
  transitive: 12
  high_risk_callers: ["main", "handle_connection"]

suggestions:
  - "Cognitive complexity 12 exceeds recommended threshold of 10"
  - "87% similar to process_request - consider consolidation"
```

**Files to modify**:
- `src/mcp_server/mod.rs` - Add tool handler
- `src/mcp_server/types.rs` - Add request/response types
- `src/risk.rs` - Expose complexity calculation as public API

---

## Phase 5: Automatic Partial Reindexing

### Enhancement: Transparent staleness handling

**Purpose**: Eliminate stale index issues without manual `check_index` calls.

**Approach**:
```
MCP Request (any query tool)
    │
    ▼
Quick staleness check (~10ms)
    │
    ├─ Fresh → Proceed with query
    │
    └─ Stale → Partial reindex (changed files only)
              │
              ▼
           Proceed with query
```

**Implementation**:
1. On any query tool (`search_symbols`, `get_repo_overview`, etc.):
   - Check index mtime vs git status
   - If stale, identify changed files via `git diff --name-only`
   - Reindex only those files
   - Update affected module shards

2. Add `auto_refresh` behavior as default (already exists in `check_index`)

**Files to modify**:
- `src/cache.rs` - Add `quick_staleness_check()` method
- `src/shard.rs` - Add `partial_reindex(changed_files)` method
- `src/mcp_server/mod.rs` - Call staleness check in query tools

**Performance target**: <50ms overhead for typical cases (1-10 changed files)

---

## Phase 6: Improved Tool Documentation for AI

### Enhancement: MCP tool descriptions optimized for agent understanding

**Current problem**: Tool descriptions don't tell agents WHEN to use them.

**Proposed improvements**:

| Tool | Current Description | Improved Description |
|------|--------------------|--------------------|
| `check_duplicates` | "Check if a specific function has duplicates..." | "**Use before writing new functions** to avoid duplication. Returns similar existing functions. Also useful for refactoring to find consolidation candidates." |
| `get_callers` | "Get callers of a symbol..." | "**Use before modifying existing code** to understand impact radius. Shows what will break if you change this function." |
| `find_duplicates` | "Find all duplicate function clusters..." | "Find code duplication across the entire codebase. Fast even on massive repos. Use for codebase health audits or before major refactoring." |
| `get_call_graph` | "Get the call graph..." | "Understand code flow and dependencies. **Use with filters** (module, symbol) for targeted analysis. Unfiltered output can be large." |

**Files to modify**:
- `src/mcp_server/mod.rs` - Update `#[tool(description = "...")]` attributes

---

## Implementation Order

| Phase | Effort | Impact | Priority |
|-------|--------|--------|----------|
| 1. `get_context` | Low | High | P0 |
| 2. Repo summary improvements | Low | High | P0 |
| 6. Tool documentation | Low | Medium | P0 |
| 5. Auto partial reindex | Medium | High | P1 |
| 4. `validate_symbol` | Medium | Medium | P1 |
| 3. BM25 semantic search | High | High | P2 |

---

## Success Metrics

1. **Token reduction**: First-contact context from 5000+ to <500 tokens
2. **Staleness elimination**: Zero manual `check_index` calls needed
3. **Quality adoption**: Agents use `validate_symbol` after code analysis
4. **Search relevance**: BM25 finds related code that exact match misses

---

## Open Questions

1. Should `get_context` be a separate tool or merged into `get_repo_summary`?
2. BM25 index size estimate for large repos - acceptable?
3. Should partial reindex be opt-out via parameter?
4. `validate_symbol` - should it auto-run `find_duplicates` or require explicit call?
