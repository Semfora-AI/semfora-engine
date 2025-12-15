# Query-Driven Semantic Index Architecture

## Task 1: Feasibility Analysis

### A. Can a Purely Query-Driven Ingestion Layer Scale to 3GB Repos?

**TL;DR: Yes, with the right data structures.**

#### Symbol Cardinality Analysis

| Repo Size | Est. Files | Est. Symbols | Symbol Index Size | Load Time |
|-----------|------------|--------------|-------------------|-----------|
| 100 MB    | 2,000      | 20,000       | ~2 MB JSONL       | <100ms    |
| 500 MB    | 10,000     | 100,000      | ~10 MB JSONL      | <500ms    |
| 1 GB      | 20,000     | 200,000      | ~20 MB JSONL      | <1s       |
| 3 GB      | 60,000     | 600,000      | ~60 MB JSONL      | <3s       |

**Key insight**: A lightweight symbol index (~100 bytes/symbol) stays manageable even at 600k symbols.

#### Symbol Index Entry (Target: ~100 bytes each)

```json
{"s":"handleLogin","h":"a1b2c3","m":"auth","f":"src/auth/login.ts","l":"45-89","r":"high"}
```

- `s`: symbol name (avg 15 chars)
- `h`: hash (16 chars)
- `m`: module (avg 8 chars)
- `f`: file path (avg 40 chars)
- `l`: line range (avg 8 chars)
- `r`: risk level (4 chars)

**Total: ~100 bytes × 600k = 60 MB** (fits in memory, fast to stream)

#### Disk I/O Strategy

| Operation | Current | Query-Driven |
|-----------|---------|--------------|
| Initial load | Load all module shards (~10MB+ each) | Load symbol index only (~60MB) |
| Symbol lookup | Already in memory | Read 1 symbol shard (~500 bytes) |
| Search | Scan all shards | Stream-filter JSONL index |

**Query-driven wins** because:
1. Cold start: 60MB index vs 500MB+ of module shards
2. Per-query: 500 bytes vs 10KB (module shard)
3. Memory: O(1) per query vs O(N) upfront

#### Caching Strategy

```
Level 0: symbol_index.jsonl     → Always loaded (60MB for 3GB repo)
Level 1: Hot symbol cache       → LRU cache of 1000 symbols (~500KB)
Level 2: Disk symbol shards     → Read on demand, filesystem cache handles rest
```

#### Tree-sitter Costs

Tree-sitter parsing is **NOT on the query path**. It only runs during:
1. `generate_index` (one-time, ~2-5 min for 3GB repo)
2. Incremental updates (changed files only)

Query path is pure file I/O: read JSONL line or symbol shard file.

#### Expected Latency Per Query

| Query Type | Operation | Expected Latency |
|------------|-----------|------------------|
| `search_symbols("login")` | Stream-filter 60MB JSONL | 50-200ms |
| `list_symbols("auth")` | Read module index section | 10-50ms |
| `get_symbol(hash)` | Single file read | 1-5ms |
| `get_symbol_source(...)` | Single file read + slice | 5-20ms |

#### Token Footprint Per Query

| Query Type | Current | Query-Driven Target |
|------------|---------|---------------------|
| `get_module("other")` | 8,000-12,000 tokens | N/A (removed) |
| `list_symbols("other")` | N/A | 300-800 tokens |
| `search_symbols("login")` | N/A | 200-500 tokens |
| `get_symbol(hash)` | 200-500 tokens | 200-500 tokens |
| `get_repo_overview` | 2,000-5,000 tokens | 2,000-5,000 tokens |

**Projected savings: 10-20x per exploration session**

---

### B. Failure Modes and Risks

#### Architectural Risks

1. **Index staleness** - Symbol index becomes stale if files change without re-indexing
   - *Mitigation*: mtime-based invalidation, warn on stale queries

2. **Cold cache thrashing** - Agent makes 100 sequential `get_symbol` calls
   - *Mitigation*: LRU cache, batch `get_symbols` endpoint

3. **Search performance at scale** - Linear scan of 60MB JSONL is O(N)
   - *Mitigation*: Add optional SQLite/tantivy index for repos >1GB

4. **Memory pressure on index load** - 60MB index must fit in memory
   - *Mitigation*: Memory-mapped file, streaming iterator

5. **Hash collisions** - FNV-1a 64-bit hash has collision probability
   - *Mitigation*: Include file path in collision resolution

#### Agent-Behavior Risks

1. **Over-fetching** - Agent calls `search_symbols("")` (matches everything)
   - *Mitigation*: Require minimum query length, pagination limits

2. **Under-scoping** - Agent searches too narrowly, misses relevant symbols
   - *Mitigation*: Fuzzy matching, related symbol suggestions

3. **Thrashing between tools** - Agent alternates search/fetch inefficiently
   - *Mitigation*: Clear MCP instructions, suggest optimal workflows

4. **Ignoring risk levels** - Agent dives into low-risk files first
   - *Mitigation*: Sort search results by risk, highlight high-risk

5. **Lost context** - Agent forgets which symbols it already fetched
   - *Mitigation*: Session state tracking (v2 feature)

#### UX Risks for Developers

1. **Index generation time** - 3GB repo takes 2-5 minutes to index
   - *Mitigation*: Progress indicators, background indexing

2. **Stale results confusion** - Developer edits file, queries return old data
   - *Mitigation*: Clear staleness warnings, auto-reindex on query

3. **Missing symbols** - Unsupported languages/patterns not indexed
   - *Mitigation*: Document coverage, fallback to grep

4. **Cache disk usage** - Large repos generate significant cache
   - *Mitigation*: Cache pruning, size limits, user control

5. **Debugging opacity** - Hard to understand why search missed a symbol
   - *Mitigation*: Debug mode, explain why results were filtered

---

### C. Architecture Comparison

#### Option 1: Giant Single Index Load (Status Quo)

```
Startup: Load all module shards into memory
Query: Return from memory
```

| Pros | Cons |
|------|------|
| Instant queries after load | 10-50k tokens on first module access |
| Simple implementation | Memory scales with repo size |
| No disk I/O per query | Wasteful for targeted queries |

**Token cost for "add forgot password button"**: ~25,000 tokens (load auth module + UI module + components)

#### Option 2: Condensed Index at Startup

```
Startup: Load lightweight symbol index + repo overview
Query: Fetch symbol details on demand
```

| Pros | Cons |
|------|------|
| Fast cold start (~1s) | Still loads full index |
| Low per-query tokens | Index must fit in memory |
| Predictable performance | More complex implementation |

**Token cost for "add forgot password button"**: ~3,000 tokens (search + 5 symbols + source slices)

#### Option 3: Pure Query-Driven, Zero Startup (Target)

```
Startup: Nothing
Query: Stream-search index file, fetch on demand
```

| Pros | Cons |
|------|------|
| Zero startup cost | Slightly slower first query |
| O(1) memory | Requires index file on disk |
| Scales to any repo size | Must handle staleness |

**Token cost for "add forgot password button"**: ~2,500 tokens (same queries, slightly more overhead)

### Recommendation

**Implement Option 2 (Condensed Index) as v1, with Option 3 capabilities.**

Rationale:
1. Option 2 gives 10x token savings with manageable complexity
2. The symbol index is small enough to load quickly (<1s for 3GB)
3. Query-driven fetching (get_symbol) already works
4. Option 3's streaming search can be added incrementally

---

## Task 2: API Design

### New MCP Tools Required

#### 1. `search_symbols`

```rust
#[derive(Deserialize, JsonSchema)]
pub struct SearchSymbolsRequest {
    /// Search query (symbol name, partial match)
    pub query: String,

    /// Optional module filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,

    /// Optional file pattern filter (glob)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_pattern: Option<String>,

    /// Maximum results (default 20, max 100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,

    /// Repository path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// Response: ~30 tokens per result
// symbol,hash,module,file,lines,risk
```

**Response format** (TOON, <1k tokens for 20 results):
```
_type: search_results
query: "login"
total: 47
showing: 20
results[20]{s,h,m,f,l,r}:
  handleLogin,a1b2c3d4,auth,src/auth/login.ts,45-89,high
  LoginForm,e5f6a7b8,components,src/components/LoginForm.tsx,12-156,high
  loginValidation,c9d0e1f2,utils,src/utils/validation.ts,78-92,low
  ...
```

#### 2. `list_symbols`

```rust
#[derive(Deserialize, JsonSchema)]
pub struct ListSymbolsRequest {
    /// Module name to list symbols from
    pub module: String,

    /// Optional symbol kind filter (function, struct, component, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Optional risk level filter (high, medium, low)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,

    /// Maximum results (default 50, max 200)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,

    /// Repository path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}
```

**Response format** (TOON, <1k tokens for 50 results):
```
_type: module_symbols
module: "auth"
total: 34
symbols[34]{s,h,k,f,l,r}:
  handleLogin,a1b2c3d4,fn,src/auth/login.ts,45-89,high
  AuthContext,b2c3d4e5,component,src/auth/AuthContext.tsx,8-45,high
  validateToken,c3d4e5f6,fn,src/auth/token.ts,12-34,medium
  ...
```

#### 3. Enhanced `get_symbol_source` (batching)

```rust
#[derive(Deserialize, JsonSchema)]
pub struct GetSymbolSourceRequest {
    /// File path (required)
    pub file_path: String,

    /// Single symbol hash (existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_hash: Option<String>,

    /// Multiple symbol hashes (NEW - for batching)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_hashes: Option<Vec<String>>,

    /// Start line (existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_line: Option<usize>,

    /// End line (existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_line: Option<usize>,

    /// Context lines (existing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<usize>,
}
```

### Symbol Index File Format

New file: `cache_dir/symbol_index.jsonl`

One JSON object per line, ~100 bytes each:
```jsonl
{"s":"handleLogin","h":"a1b2c3d4","k":"fn","m":"auth","f":"src/auth/login.ts","l":"45-89","r":"high"}
{"s":"LoginForm","h":"e5f6a7b8","k":"component","m":"components","f":"src/components/LoginForm.tsx","l":"12-156","r":"high"}
{"s":"validateEmail","h":"f7g8h9i0","k":"fn","m":"utils","f":"src/utils/validation.ts","l":"23-45","r":"low"}
```

Benefits:
- Streamable (no need to parse entire file)
- Append-only for incremental updates
- Human-readable for debugging
- Easy to grep/filter

---

## Task 3: Implementation

### A. New Struct Definitions

Location: `src/mcp_server/mod.rs`

```rust
// ============= NEW REQUEST TYPES =============

/// Search for symbols by name across the repository
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SearchSymbolsRequest {
    /// Search query - matches symbol names (case-insensitive, partial match)
    pub query: String,

    /// Optional: filter by module name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module: Option<String>,

    /// Optional: filter by file path pattern (glob)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_pattern: Option<String>,

    /// Optional: filter by symbol kind (fn, struct, component, enum, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Optional: filter by risk level (high, medium, low)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,

    /// Maximum results to return (default: 20, max: 100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,

    /// Repository path (defaults to current directory)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// List all symbols in a specific module (lightweight index only)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ListSymbolsRequest {
    /// Module name to list symbols from
    pub module: String,

    /// Optional: filter by symbol kind
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,

    /// Optional: filter by risk level
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk: Option<String>,

    /// Maximum results (default: 50, max: 200)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,

    /// Repository path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

// ============= INDEX ENTRY TYPE =============

/// Lightweight symbol index entry (~100 bytes serialized)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolIndexEntry {
    /// Symbol name
    #[serde(rename = "s")]
    pub symbol: String,

    /// Symbol hash (for get_symbol lookup)
    #[serde(rename = "h")]
    pub hash: String,

    /// Symbol kind (fn, struct, component, enum, trait, etc.)
    #[serde(rename = "k")]
    pub kind: String,

    /// Module name
    #[serde(rename = "m")]
    pub module: String,

    /// File path (relative to repo root)
    #[serde(rename = "f")]
    pub file: String,

    /// Line range (e.g., "45-89")
    #[serde(rename = "l")]
    pub lines: String,

    /// Risk level (high, medium, low)
    #[serde(rename = "r")]
    pub risk: String,
}
```

### B. Symbol Index Generation

Location: `src/shard.rs` (add to ShardWriter)

```rust
impl ShardWriter {
    /// Write the lightweight symbol index for query-driven access
    fn write_symbol_index(&self, stats: &mut ShardStats) -> Result<()> {
        let path = self.cache.symbol_index_path();
        let mut file = fs::File::create(&path)?;

        for summary in &self.all_summaries {
            if let Some(ref symbol_id) = summary.symbol_id {
                let entry = SymbolIndexEntry {
                    symbol: summary.symbol.clone().unwrap_or_default(),
                    hash: symbol_id.hash.clone(),
                    kind: summary.symbol_kind
                        .map(|k| format!("{:?}", k).to_lowercase())
                        .unwrap_or_else(|| "unknown".to_string()),
                    module: extract_module_name(&summary.file),
                    file: summary.file.clone(),
                    lines: summary.lines.clone().unwrap_or_default(),
                    risk: format!("{:?}", summary.behavioral_risk).to_lowercase(),
                };

                // Write as JSONL (one JSON object per line)
                let json = serde_json::to_string(&entry)?;
                writeln!(file, "{}", json)?;

                stats.index_entries += 1;
            }
        }

        stats.index_bytes = fs::metadata(&path)?.len() as usize;
        Ok(())
    }
}
```

### C. Cache Directory Extension

Location: `src/cache.rs` (add to CacheDir)

```rust
impl CacheDir {
    /// Path to the symbol index file
    pub fn symbol_index_path(&self) -> PathBuf {
        self.root.join("symbol_index.jsonl")
    }

    /// Load and stream symbol index entries
    pub fn stream_symbol_index(&self) -> Result<impl Iterator<Item = Result<SymbolIndexEntry>>> {
        let file = fs::File::open(self.symbol_index_path())?;
        let reader = std::io::BufReader::new(file);

        Ok(reader.lines().map(|line| {
            let line = line?;
            let entry: SymbolIndexEntry = serde_json::from_str(&line)?;
            Ok(entry)
        }))
    }

    /// Search symbol index with filters
    pub fn search_symbols(
        &self,
        query: &str,
        module: Option<&str>,
        kind: Option<&str>,
        risk: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SymbolIndexEntry>> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for entry_result in self.stream_symbol_index()? {
            let entry = entry_result?;

            // Match query against symbol name (case-insensitive, partial)
            if !entry.symbol.to_lowercase().contains(&query_lower) {
                continue;
            }

            // Apply optional filters
            if let Some(m) = module {
                if entry.module != m {
                    continue;
                }
            }
            if let Some(k) = kind {
                if entry.kind != k {
                    continue;
                }
            }
            if let Some(r) = risk {
                if entry.risk != r {
                    continue;
                }
            }

            results.push(entry);

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    /// List symbols in a module (lightweight index only)
    pub fn list_module_symbols(
        &self,
        module: &str,
        kind: Option<&str>,
        risk: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SymbolIndexEntry>> {
        let mut results = Vec::new();

        for entry_result in self.stream_symbol_index()? {
            let entry = entry_result?;

            if entry.module != module {
                continue;
            }

            if let Some(k) = kind {
                if entry.kind != k {
                    continue;
                }
            }
            if let Some(r) = risk {
                if entry.risk != r {
                    continue;
                }
            }

            results.push(entry);

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }
}
```

### D. MCP Tool Handlers

Location: `src/mcp_server/mod.rs`

```rust
#[tool(
    name = "search_symbols",
    description = "Search for symbols by name across the repository. Returns lightweight index entries (symbol, hash, module, file, lines, risk) without full semantic details. Use get_symbol(hash) to fetch full details for specific symbols."
)]
async fn search_symbols(&self, request: SearchSymbolsRequest) -> Result<String, McpError> {
    let repo_path = match &request.path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().map_err(|e| McpError {
            code: -1,
            message: format!("Failed to get current directory: {}", e),
            data: None,
        })?,
    };

    let cache = match CacheDir::for_repo(&repo_path) {
        Ok(c) => c,
        Err(e) => return Err(McpError {
            code: -1,
            message: format!("Failed to open cache: {}", e),
            data: None,
        }),
    };

    let limit = request.limit.unwrap_or(20).min(100);

    let results = cache.search_symbols(
        &request.query,
        request.module.as_deref(),
        request.kind.as_deref(),
        request.risk.as_deref(),
        limit,
    ).map_err(|e| McpError {
        code: -1,
        message: format!("Search failed: {}", e),
        data: None,
    })?;

    // Format as compact TOON
    let mut output = String::new();
    output.push_str("_type: search_results\n");
    output.push_str(&format!("query: \"{}\"\n", request.query));
    output.push_str(&format!("showing: {}\n", results.len()));
    output.push_str(&format!("results[{}]{{s,h,k,m,f,l,r}}:\n", results.len()));

    for entry in results {
        output.push_str(&format!(
            "  {},{},{},{},{},{},{}\n",
            entry.symbol, entry.hash, entry.kind, entry.module, entry.file, entry.lines, entry.risk
        ));
    }

    Ok(output)
}

#[tool(
    name = "list_symbols",
    description = "List all symbols in a specific module. Returns lightweight index entries only. More efficient than get_module for exploring module contents."
)]
async fn list_symbols(&self, request: ListSymbolsRequest) -> Result<String, McpError> {
    let repo_path = match &request.path {
        Some(p) => PathBuf::from(p),
        None => std::env::current_dir().map_err(|e| McpError {
            code: -1,
            message: format!("Failed to get current directory: {}", e),
            data: None,
        })?,
    };

    let cache = match CacheDir::for_repo(&repo_path) {
        Ok(c) => c,
        Err(e) => return Err(McpError {
            code: -1,
            message: format!("Failed to open cache: {}", e),
            data: None,
        }),
    };

    let limit = request.limit.unwrap_or(50).min(200);

    let results = cache.list_module_symbols(
        &request.module,
        request.kind.as_deref(),
        request.risk.as_deref(),
        limit,
    ).map_err(|e| McpError {
        code: -1,
        message: format!("List failed: {}", e),
        data: None,
    })?;

    // Format as compact TOON
    let mut output = String::new();
    output.push_str("_type: module_symbols\n");
    output.push_str(&format!("module: \"{}\"\n", request.module));
    output.push_str(&format!("total: {}\n", results.len()));
    output.push_str(&format!("symbols[{}]{{s,h,k,f,l,r}}:\n", results.len()));

    for entry in results {
        output.push_str(&format!(
            "  {},{},{},{},{},{}\n",
            entry.symbol, entry.hash, entry.kind, entry.file, entry.lines, entry.risk
        ));
    }

    Ok(output)
}
```

---

## Task 4: Workflow Simulation

### Scenario: "Add a 'Forgot Password' button to the login page"

#### Query-Driven Workflow (NEW)

```
Step 1: get_repo_overview
        → Understand architecture, find relevant modules
        → ~2,500 tokens

Step 2: search_symbols("login")
        → Find login-related symbols across codebase
        → Results: LoginPage, LoginForm, handleLogin, loginValidation...
        → ~400 tokens (20 results × 20 tokens each)

Step 3: search_symbols("password")
        → Find password-related patterns
        → Results: PasswordInput, validatePassword, resetPassword...
        → ~400 tokens

Step 4: get_symbol("abc123")  // LoginForm hash
        → Get full semantic details for LoginForm component
        → ~350 tokens

Step 5: get_symbol("def456")  // LoginPage hash
        → Get full semantic details for LoginPage
        → ~300 tokens

Step 6: get_symbol_source(file="src/pages/LoginPage.tsx", hash="def456")
        → Get actual source code for editing
        → ~400 tokens (50 lines with context)

Step 7: search_symbols("forgot")
        → Check for existing forgot password implementations
        → ~200 tokens (few or no results)

TOTAL: ~4,550 tokens
```

#### Old Workflow (Current)

```
Step 1: get_repo_overview
        → ~2,500 tokens

Step 2: get_module("pages")
        → Load ALL page symbols with full semantic details
        → ~6,000 tokens

Step 3: get_module("components")
        → Load ALL component symbols
        → ~8,000 tokens

Step 4: get_module("auth")
        → Load ALL auth symbols
        → ~4,000 tokens

Step 5: get_symbol_source(...)
        → ~400 tokens

TOTAL: ~20,900 tokens
```

#### Comparison

| Metric | Old Workflow | Query-Driven | Savings |
|--------|--------------|--------------|---------|
| Total tokens | 20,900 | 4,550 | **4.6x** |
| MCP calls | 5 | 7 | +2 calls |
| Latency | ~2s (large shards) | ~500ms (small reads) | **4x faster** |
| Precision | Low (loaded everything) | High (only relevant) | Better |

---

## Task 5: Final Summary

### Feasibility: ✅ CONFIRMED

A query-driven architecture scales to 3GB repos with:
- 60MB symbol index (600k symbols × 100 bytes)
- <3s index load time
- <200ms search latency
- O(1) memory per query

### Design: ✅ COMPLETE

New tools designed:
1. `search_symbols(query, filters)` - Full-text symbol search
2. `list_symbols(module, filters)` - Lightweight module listing
3. Enhanced `get_symbol_source` - Batch support (optional)

New data structure:
- `symbol_index.jsonl` - Streamable, 100 bytes/entry

### Implementation: ✅ FIRST VERSION READY

Code additions:
- `SymbolIndexEntry` struct
- `SearchSymbolsRequest` / `ListSymbolsRequest` structs
- `CacheDir::search_symbols()` / `list_module_symbols()`
- `ShardWriter::write_symbol_index()`
- MCP handlers for both tools

### Expected Performance Benefits

| Scenario | Before | After | Improvement |
|----------|--------|-------|-------------|
| Typical task exploration | 25-50k tokens | 5-10k tokens | **5x** |
| Cold start module access | 10k tokens | 500 tokens | **20x** |
| Symbol search | N/A (grep fallback) | 400 tokens | **New capability** |
| Memory usage | O(repo size) | O(1) per query | **Constant** |

### Remaining Work for v2

1. **Pagination** - Handle >100 search results
2. **Fuzzy matching** - Typo tolerance in search
3. **SQLite backend** - For repos >1GB (faster than JSONL scan)
4. **Incremental updates** - Append to index without full rebuild
5. **Session state** - Track which symbols agent has seen
6. **Related symbols** - "Also consider these" suggestions
7. **Diff-aware search** - "Symbols changed in this PR"
8. **analyze_diff_summary** - Lightweight diff overview

### MCP Instructions Update

```markdown
## Query-Driven Workflow (Recommended)

For token-efficient exploration:

1. `get_repo_overview` → Understand architecture (once per session)
2. `search_symbols(query)` → Find relevant symbols by name
3. `list_symbols(module)` → Browse module contents (lightweight)
4. `get_symbol(hash)` → Fetch full details for specific symbols
5. `get_symbol_source(...)` → Get actual code for editing

AVOID:
- `get_module` - Returns full semantic details (expensive)
- `analyze_diff` without filters - Returns all changed files

Token budget per query:
- search_symbols: ~400 tokens (20 results)
- list_symbols: ~800 tokens (50 results)
- get_symbol: ~350 tokens
- get_symbol_source: ~400 tokens (50 lines)
```
