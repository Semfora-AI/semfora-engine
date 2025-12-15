# Technical Competitive Analysis: Semfora Engine vs LogicStamp Context

**Document Version**: 1.0
**Analysis Date**: December 14, 2025
**Analyst**: Claude Code (Opus 4.5)

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Architecture Comparison](#architecture-comparison)
3. [Language & Framework Support](#language--framework-support)
4. [Analysis Capabilities Deep Dive](#analysis-capabilities-deep-dive)
5. [Performance Characteristics](#performance-characteristics)
6. [Output Formats & Data Structures](#output-formats--data-structures)
7. [Feature Gap Analysis](#feature-gap-analysis)
8. [Strategic Recommendations](#strategic-recommendations)

---

## Executive Summary

| Dimension | LogicStamp Context | Semfora Engine |
|-----------|-------------------|----------------|
| **Primary Language** | TypeScript (Node.js) | Rust |
| **Codebase Size** | ~15,000 lines | ~52,400 lines |
| **Language Support** | 2 (TS/JS) | 26 languages |
| **Architecture** | CLI + library | CLI + MCP server + daemon |
| **Token Compression** | 65-72% | 70%+ (TOON format) |
| **Target User** | Individual React devs | Enterprise teams |

**Bottom Line**: LogicStamp is a specialized React tool optimized for quick AI context. Semfora is a comprehensive semantic analysis platform with deeper capabilities, broader language support, and better performance at scale.

---

## Architecture Comparison

### LogicStamp Context Architecture

```
┌─────────────────────────────────────────────────────────┐
│                    CLI Entry (stamp)                     │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                   Command Router                         │
│  context | compare | validate | security | init | clean │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│                   Core Pipeline                          │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌─────────┐ │
│  │ AST      │→ │ Contract │→ │ Manifest │→ │ Bundle  │ │
│  │ Parser   │  │ Builder  │  │ Builder  │  │ Packer  │ │
│  │(ts-morph)│  │(UIFv0.3) │  │(Deps)    │  │(BFS)    │ │
│  └──────────┘  └──────────┘  └──────────┘  └─────────┘ │
└─────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────┐
│               Output: context.json files                 │
└─────────────────────────────────────────────────────────┘
```

**Key Components**:
- **AST Parser**: Uses `ts-morph` (TypeScript Compiler API wrapper)
- **Contract Builder**: Generates UIFContract v0.3 schema
- **Style Extractors**: 7 framework-specific extractors (Tailwind, MUI, etc.)
- **Security Scanner**: Regex-based secret detection with sanitization
- **Token Estimator**: GPT-4 (tiktoken) and Claude tokenizers

**Technical Stack**:
```
ts-morph: ^21.0.1       # TypeScript AST manipulation
glob: ^10.3.10          # File pattern matching
@dqbd/tiktoken          # OpenAI tokenizer (optional)
@anthropic-ai/tokenizer # Claude tokenizer (optional)
Node.js >= 18.0.0       # Runtime
```

### Semfora Engine Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│                         Binary Targets                               │
│  semfora-engine (CLI) | semfora-engine-server (MCP) | semfora-daemon│
└─────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
┌─────────────────────┐ ┌─────────────────┐ ┌─────────────────────────┐
│   Single Analysis   │ │  Index-Based    │ │   Real-Time Daemon      │
│   analyze_file()    │ │  Queries        │ │   WebSocket + Watcher   │
└─────────────────────┘ └─────────────────┘ └─────────────────────────┘
                    │               │               │
                    └───────────────┼───────────────┘
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Extraction Pipeline                               │
│  ┌────────────┐  ┌────────────┐  ┌────────────┐  ┌────────────────┐│
│  │ Tree-sitter│→ │ Language   │→ │ Framework  │→ │ Risk/Analysis  ││
│  │ Parser     │  │ Detector   │  │ Enhancer   │  │ Engine         ││
│  └────────────┘  └────────────┘  └────────────┘  └────────────────┘│
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    4-Layer Index System                              │
│  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐                │
│  │  BASE   │← │ BRANCH  │← │ WORKING │← │   AI    │                │
│  │ (main)  │  │(commits)│  │(unstaged)│  │(proposed)│               │
│  └─────────┘  └─────────┘  └─────────┘  └─────────┘                │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    Output Formats                                    │
│              TOON (compressed) | JSON (structured)                   │
└─────────────────────────────────────────────────────────────────────┘
```

**Key Components (30+ modules)**:
| Module | Lines | Purpose |
|--------|-------|---------|
| `src/extract.rs` | ~800 | Main extraction orchestration |
| `src/detectors/` | ~8,000 | 16 language-specific extractors |
| `src/shard.rs` | ~1,200 | Sharded index generation |
| `src/overlay.rs` | ~2,400 | 4-layer index system (144 functions) |
| `src/drift.rs` | ~870 | SHA-based drift detection |
| `src/duplicate/` | ~1,500 | Semantic duplicate detection |
| `src/mcp_server/` | ~4,000 | MCP protocol (50+ tools) |
| `src/server/` | ~2,000 | Daemon, file watcher, git poller |
| `src/bm25.rs` | ~400 | Semantic search ranking |

**Technical Stack**:
```
tree-sitter: Multiple grammars  # AST parsing (26 languages)
rayon: Parallel processing      # Multi-core utilization
serde: Serialization            # JSON/TOON output
tokio: Async runtime            # WebSocket daemon
Pure Rust, no external deps     # Single binary deployment
```

---

## Language & Framework Support

### LogicStamp Language Support

| Language | AST Support | Semantic Analysis | Framework Detection |
|----------|-------------|-------------------|---------------------|
| TypeScript | Full (ts-morph) | Components, hooks, props, state | React, Next.js |
| JavaScript | Full (ts-morph) | Same as TypeScript | React, Next.js |
| JSX/TSX | Full | React-specific patterns | All React UI libs |
| **Others** | **None** | **None** | **None** |

**Framework Detection Details**:
- **React**: `useState`, `useEffect`, `forwardRef`, custom hooks (`useXxx`)
- **Next.js**: `'use client'`/`'use server'` directives, `/app` router, layouts, pages
- **Style Frameworks**: See dedicated section below

### Semfora Language Support

| Language Family | Languages | AST Parser | Framework Detection |
|-----------------|-----------|------------|---------------------|
| **JavaScript** | TS, JS, JSX, TSX | tree-sitter-typescript | React, Next.js, Express, Vue, Angular, Svelte, NestJS, Fastify, Hono, Remix |
| **Systems** | Rust, C, C++ | tree-sitter-{rust,c,cpp} | None |
| **Backend** | Python, Go, Java, Kotlin, C# | tree-sitter-{python,go,java,kotlin,c_sharp} | Django, Flask, FastAPI (partial) |
| **Shell** | Bash, Shell | tree-sitter-bash | None |
| **Config** | JSON, YAML, TOML, HCL | tree-sitter-{json,yaml} | Terraform |
| **Markup** | HTML, CSS, SCSS, Vue, Markdown | tree-sitter-{html,css} | None |

**Total: 26 languages with AST support**

### Style Framework Support (LogicStamp Exclusive)

LogicStamp has deep metadata extraction for:

| Framework | Detection Method | Metadata Extracted |
|-----------|------------------|-------------------|
| **Tailwind CSS** | `className` parsing, `tailwind.config` | Class categories (layout, spacing, colors, typography), breakpoints, variant count |
| **Material UI** | `@mui/*` imports | Components used, sx prop patterns, makeStyles, theme usage, system props |
| **Shadcn/UI** | Component imports | Variants, sizes, form integration, dark mode, icons, density |
| **Radix UI** | `@radix-ui/*` imports | Primitives, composition patterns (controlled/uncontrolled), accessibility features |
| **Framer Motion** | `framer-motion` imports | Motion components, variants, gestures, layout animations, viewport triggers |
| **Styled Components** | `styled-components` imports | Template literal detection, theme usage, CSS prop patterns |
| **SCSS/CSS Modules** | `.module.css`, `.scss` | Selectors, properties, variables, mixins, nesting depth |

**Example Tailwind Metadata Output**:
```json
{
  "style": {
    "tailwind": {
      "classCount": 47,
      "categories": {
        "layout": ["flex", "grid", "container"],
        "spacing": ["p-4", "m-2", "gap-3"],
        "colors": ["bg-blue-500", "text-gray-900"],
        "typography": ["text-lg", "font-bold"]
      },
      "breakpoints": ["sm", "md", "lg"],
      "darkMode": true
    }
  }
}
```

**Semfora Style Support**: None. CSS/SCSS files are parsed but no framework-specific metadata is extracted.

---

## Analysis Capabilities Deep Dive

### AST Parsing Comparison

| Capability | LogicStamp | Semfora |
|------------|------------|---------|
| **Parser Technology** | ts-morph (TypeScript Compiler API) | tree-sitter (universal) |
| **Type Information** | Full TypeScript types | Limited (AST only, no type checker) |
| **Incremental Parsing** | No | Yes (tree-sitter edit API) |
| **Error Recovery** | ts-morph built-in | tree-sitter error nodes |
| **Performance** | ~100-200ms/file | ~5-50ms/file (with caching: <1ms) |

### Semantic Analysis

**LogicStamp Extracts**:
```typescript
// UIFContract v0.3 schema
{
  kind: "react:component" | "ts:module" | "node:cli",
  version: {
    variables: string[],      // Module-level variables
    hooks: string[],          // React hooks used
    components: string[],     // Child components
    functions: string[],      // Helper functions
    imports: string[]         // Dependencies
  },
  logicSignature: {
    props: Record<string, PropType>,   // Typed props with normalization
    emits: Record<string, EventType>,  // Event handlers
    state: Record<string, string>      // State variables
  },
  exports: "default" | "named" | { named: string[] },
  prediction: string[],       // Behavioral predictions
  nextjs: { directive, isInAppDir },
  style: StyleMetadata
}
```

**Semfora Extracts**:
```rust
// SemanticSummary schema
{
    symbol: String,           // Primary symbol name
    symbol_kind: SymbolKind,  // function|class|struct|component|trait|enum|interface|type|constant|module
    visibility: Visibility,   // public|private|protected|internal|crate
    behavioral_risk: Risk,    // low|medium|high (scored)
    added_dependencies: Vec<Dependency>,  // Imports with sources
    state_changes: Vec<StateChange>,      // Variable mutations
    control_flow: Vec<ControlFlow>,       // if/for/while/match/try
    calls: Vec<FunctionCall>,             // Deduplicated calls with context
    insertions: Vec<Insertion>,           // Extracted symbols with positions
    raw_fallback: Option<String>,         // Source for unsupported languages
    line_range: (u32, u32),
    file_path: String,
    frameworks: Option<FrameworkContext>  // Detected frameworks
}
```

### Call Graph Analysis

**LogicStamp**:
- Basic dependency graph via BFS traversal
- Tracks component → component dependencies
- No reverse lookups (callers)
- No cross-module resolution beyond imports
- Depth configurable (default: 1)

```
Button → Icon
       → Tooltip
       → useTheme (hook)
```

**Semfora**:
- Full bidirectional call graph
- `get_callers(symbol)` - Who calls this function?
- `get_call_graph(module)` - All relationships in module
- Multi-depth traversal (up to 3 levels)
- Cross-module symbol resolution
- Cycle detection

```
// Semfora call graph output
{
  "edges": [
    { "caller": "handleSubmit", "callee": "validateForm" },
    { "caller": "validateForm", "callee": "checkEmail" },
    { "caller": "handleSubmit", "callee": "submitData" }
  ],
  "callers_of_validateForm": ["handleSubmit", "resetForm", "autoSave"]
}
```

### Duplicate Detection

**LogicStamp**: None

**Semfora**:
- **Algorithm**: Semantic fingerprinting + Jaccard similarity
- **Fingerprint Components**:
  - Set signature (identifiers used)
  - Control flow patterns
  - State mutations
  - Call patterns
- **Boilerplate Filtering**: 27 patterns (14 JS/TS, 13 Rust)
  - JS: ReactQuery, Redux, Zod schemas, React hooks, etc.
  - Rust: Builder patterns, trait impls, From/Into, getters/setters
- **Similarity Threshold**: Configurable (default: 0.90)
- **Output**: Clusters of similar functions with diff enumeration

```json
{
  "clusters": [
    {
      "similarity": 0.94,
      "functions": [
        { "symbol": "handleUserSubmit", "file": "src/user.ts:45" },
        { "symbol": "handleOrderSubmit", "file": "src/order.ts:78" }
      ],
      "differences": [
        "Different API endpoint",
        "Order has additional validation"
      ]
    }
  ]
}
```

### Risk Assessment

**LogicStamp**: None

**Semfora Risk Calculation** (`src/risk.rs`):

| Factor | Points | Detection Method |
|--------|--------|------------------|
| Many imports | +1 | `dependencies.len() > 5` |
| State mutations | +1 | `state_changes.len() > 3` |
| Complex control flow | +1 | `control_flow.len() > 5` |
| I/O operations | +2 | String match: "fetch", "read", "write", "http" |
| Persistence ops | +1 | String match: "database", "storage", "persist" |
| Public API changes | +1 | Visibility == Public && has modifications |

**Risk Levels**:
- **Low**: 0-2 points
- **Medium**: 3-4 points
- **High**: 5+ points

---

## Performance Characteristics

### LogicStamp Performance

| Metric | Value | Notes |
|--------|-------|-------|
| **Single file analysis** | ~100-200ms | ts-morph compilation |
| **Full repo scan** | Linear with file count | No parallelization |
| **Incremental updates** | None | Full rescan required |
| **Memory usage** | Unknown | Node.js heap |
| **Large repo warning** | "May be slower" (docs) | 10k+ files problematic |

**Performance Bottlenecks**:
1. ts-morph creates TypeScript program per file
2. No AST caching between runs
3. Single-threaded execution
4. Full re-analysis on any change

### Semfora Performance

| Metric | Value | Notes |
|--------|-------|-------|
| **Single file (cold)** | ~5-50ms | tree-sitter parsing |
| **Single file (cached)** | ~0.096ms (96μs) | AST cache hit |
| **Small file speedup** | 3.5x | ~700 bytes |
| **Large file speedup** | 25x | ~49KB |
| **Cache hit speedup** | 760x | Hot cache |
| **Incremental update** | <500ms | <10 changed files |
| **Memory per file** | ~2-5KB | Source + AST cached |
| **Parallelization** | Full | rayon work-stealing |

**Benchmarked Operations** (from `docs/performance.md`):

```
Indexing 1000 files:
  - Cold start: 2.1s
  - With caching: 0.4s

Single file update:
  - Detect change: 10ms (SHA comparison)
  - Re-parse file: 5-50ms
  - Update index: 50ms
  - Total: <100ms

Query operations:
  - Symbol lookup: O(1) via JSONL index
  - Search 10k symbols: ~50ms
  - BM25 search: ~100ms
  - Call graph (depth=3): ~200ms
```

**Drift Detection Strategy** (`src/drift.rs`):

| Changed Files | Strategy | Rebuild Scope |
|---------------|----------|---------------|
| 0 | Fresh | None |
| 1-10 | Incremental | Changed files only |
| 11-30% of repo | Rebase overlay | Overlay rebuild |
| >30% of repo | Full rebuild | Complete reindex |

### Token Compression Comparison

| Input | LogicStamp | Semfora |
|-------|------------|---------|
| **Raw source** (1000 tokens) | ~300-350 tokens (65-72%) | ~300 tokens (70%+) |
| **Repository overview** | Not applicable | 15,000 → 4,000 tokens |
| **Symbol list (50 items)** | Not applicable | 2,400 → 800 tokens |
| **Format** | JSON (minified/pretty) | TOON (text-optimized) |

---

## Output Formats & Data Structures

### LogicStamp Output Schema

**LogicStampBundle (v0.1)**:
```typescript
interface LogicStampBundle {
  type: "LogicStampBundle";
  schemaVersion: "0.1";
  entryId: string;              // "src/components/Button.tsx"
  depth: number;                // Traversal depth used
  createdAt: string;            // ISO 8601 timestamp
  bundleHash: string;           // "uif:abc123..."
  graph: {
    nodes: BundleNode[];        // Component contracts
    edges: [string, string][];  // [from, to] dependencies
  };
  meta: {
    missing: { name: string; reason: string }[];
    source: string;             // Tool version
  };
}
```

**UIFContract (v0.3)**:
```typescript
interface UIFContract {
  type: "UIFContract";
  schemaVersion: "0.3";
  kind: "react:component" | "ts:module" | "node:cli";
  entryId: string;
  description?: string;
  version: {
    variables: string[];
    hooks: string[];
    components: string[];
    functions: string[];
    imports: string[];
  };
  logicSignature: {
    props: Record<string, PropType>;
    emits: Record<string, EventType>;
    state?: Record<string, string>;
  };
  exports?: "default" | "named" | { named: string[] };
  prediction?: string[];        // ["submit-only", "display-only", "nav-only"]
  nextjs?: {
    directive: "client" | "server";
    isInAppDir: boolean;
  };
  style?: StyleMetadata;
  semanticHash: string;         // Content-based hash
  fileHash: string;             // Source file hash
}
```

**Output Files**:
- `context.json` - Per-folder bundle
- `context_main.json` - Project index
- `stamp_security_report.json` - Secret detection results
- `.stampignore` - Exclusion patterns
- `.logicstamprc` - Configuration

### Semfora Output Schema

**TOON Format** (human-readable, compressed):
```
[fn] handleSubmit @ src/form.ts:45-89 | risk:high
  deps: react, axios, ./validation
  state: formData:FormState, isLoading:bool, error:string|null
  flow: if→try→await→catch
  calls: validateForm(), submitData(await,try), setError()

[component] LoginForm @ src/LoginForm.tsx:1-120 | risk:medium
  deps: react, ./Button, ./Input, ./hooks/useAuth
  hooks: useState(3), useEffect(1), useAuth(custom)
  state: email:string, password:string, rememberMe:bool
  flow: if→try→await
  calls: handleSubmit(), validateEmail(), login(await,try)
```

**JSON Format**:
```json
{
  "symbol": "handleSubmit",
  "symbol_kind": "function",
  "visibility": "private",
  "behavioral_risk": "high",
  "line_range": [45, 89],
  "file_path": "src/form.ts",
  "added_dependencies": [
    { "name": "react", "source": "external" },
    { "name": "axios", "source": "external" },
    { "name": "./validation", "source": "local" }
  ],
  "state_changes": [
    { "name": "formData", "type": "FormState", "initializer": "useState" },
    { "name": "isLoading", "type": "bool", "initializer": "false" },
    { "name": "error", "type": "string|null", "initializer": "null" }
  ],
  "control_flow": ["if", "try", "await", "catch"],
  "calls": [
    { "name": "validateForm", "await": false, "try": false },
    { "name": "submitData", "await": true, "try": true },
    { "name": "setError", "await": false, "try": false }
  ]
}
```

**Index Structure**:
```
.semfora-cache/
├── repo_overview.toon      # High-level summary
├── modules/
│   ├── api.toon            # Module shard
│   ├── components.toon
│   └── utils.toon
├── symbols.jsonl           # O(1) symbol lookup
├── call_graph.json         # Function relationships
├── import_graph.json       # Module dependencies
└── cache_meta.json         # Staleness tracking
```

---

## Feature Gap Analysis

### Gap 1: Secret Detection

**What LogicStamp Has**:
```typescript
// Patterns detected (from security.ts)
const SECRET_PATTERNS = [
  /api[_-]?key/i,
  /secret[_-]?key/i,
  /password/i,
  /auth[_-]?token/i,
  /bearer[_-]?token/i,
  /database[_-]?url/i,
  /connection[_-]?string/i,
  /private[_-]?key/i,
  /AWS_SECRET_ACCESS_KEY/,
  /GITHUB_TOKEN/,
  // ... 20+ patterns
];

// Output sanitization
function sanitize(value: string): string {
  return "PRIVATE_DATA";
}
```

**Semfora Implementation Plan**:

| Step | File | Implementation | Performance Impact |
|------|------|----------------|-------------------|
| 1 | `src/detectors/secrets.rs` | Regex patterns (lazy_static) | +1-2ms per file |
| 2 | `src/schema.rs` | Add `SecretFinding` to `SemanticSummary` | Negligible |
| 3 | `src/risk.rs` | `secrets_detected.len() > 0` → +3 risk | Negligible |
| 4 | `src/toon.rs` | Sanitize in TOON output | Negligible |
| 5 | `src/mcp_server/mod.rs` | `security_scan` tool | On-demand only |

**Estimated Effort**: 2-3 days

---

### Gap 2: CI/CD Validation

**What LogicStamp Has**:
```bash
# Hash locking
stamp context --hash-lock
# Creates .stamp-lock.json with file hashes

# Strict mode (CI)
stamp context --strict
# Fails on: missing dependencies, unresolved imports

# Validation
stamp context validate
# Validates schema, checks hashes against lock file

# Comparison
stamp context compare --baseline main
# Shows drift from baseline
```

**Semfora Implementation Plan**:

| Step | File | Implementation | Performance Impact |
|------|------|----------------|-------------------|
| 1 | `src/cli.rs` | `validate` subcommand | On-demand |
| 2 | `src/validation.rs` | Schema + hash validation | O(n) files |
| 3 | `src/cli.rs` | `--strict` flag (already defined) | Negligible |
| 4 | `src/cli.rs` | `compare` subcommand (use `analyze_diff`) | Already exists |
| 5 | `src/mcp_server/mod.rs` | `validate_index` tool | On-demand |

**Existing Infrastructure**:
- `src/drift.rs` (870 lines): Full SHA-based drift detection
- `src/cache.rs` (380 lines): FNV-1a stable hashing
- `analyze_diff` MCP tool: Already compares branches

**Estimated Effort**: 1-2 days (mostly CLI wiring)

---

### Gap 3: Style Framework Detection

**What LogicStamp Has** (detailed):

```typescript
// Tailwind extractor (tailwind.ts)
interface TailwindMetadata {
  classCount: number;
  categories: {
    layout: string[];      // flex, grid, container
    spacing: string[];     // p-*, m-*, gap-*
    colors: string[];      // bg-*, text-*, border-*
    typography: string[];  // text-*, font-*
    sizing: string[];      // w-*, h-*, max-*
    effects: string[];     // shadow-*, opacity-*
    transitions: string[]; // transition-*, duration-*
  };
  breakpoints: string[];   // sm, md, lg, xl, 2xl
  darkMode: boolean;
  customClasses: string[]; // Non-standard patterns
}

// MUI extractor (material.ts)
interface MUIMetadata {
  components: string[];    // Button, TextField, etc.
  packages: string[];      // @mui/material, @mui/icons
  features: {
    theme: boolean;        // useTheme, ThemeProvider
    sx: boolean;           // sx prop usage
    makeStyles: boolean;   // Legacy API
    systemProps: boolean;  // spacing, display props
  };
}
```

**Semfora Implementation Plan**:

| Step | File | Implementation | Performance Impact |
|------|------|----------------|-------------------|
| 1 | `src/detectors/javascript/frameworks/mod.rs` | Add `StyleFramework` enum | Negligible |
| 2 | `src/detectors/javascript/frameworks/styles.rs` | Detection via imports | +0.5ms per JS file |
| 3 | `src/detectors/javascript/frameworks/tailwind.rs` | Class parsing | +2-5ms if Tailwind detected |
| 4 | `src/detectors/javascript/frameworks/mui.rs` | sx/theme detection | +1-2ms if MUI detected |
| 5 | `src/schema.rs` | Add `style_frameworks: Vec<StyleFramework>` | Negligible |

**Existing Infrastructure**:
- `src/detectors/javascript/frameworks/` has perfect pattern to follow
- Import-based detection already implemented for React, Vue, Angular
- `FrameworkContext` struct already exists

**Estimated Effort**:
- Basic detection: 1-2 days
- Deep metadata (Tailwind classes, etc.): 3-5 additional days

---

### Gap 4: Zero-Config Experience

**What LogicStamp Has**:
```bash
# Works immediately
stamp context  # No setup required

# Profiles for common use cases
stamp context --profile llm-chat    # depth=1, header mode, max 100 nodes
stamp context --profile llm-safe    # depth=1, header mode, max 30 nodes
stamp context --profile ci-strict   # no code, strict validation
```

**Semfora Current State**:
- `analyze_file()` works without index ✓
- `analyze_directory()` works without index ✓
- `ensure_fresh_index()` auto-generates when stale ✓
- Most MCP tools require index generation first ✗

**Semfora Implementation Plan**:

| Step | File | Implementation | Performance Impact |
|------|------|----------------|-------------------|
| 1 | `src/mcp_server/mod.rs` | Auto-index on first query | One-time 2-5s |
| 2 | `src/config.rs` | Profile presets (minimal, full, ci) | Negligible |
| 3 | `src/cli.rs` | `--profile` flag | Negligible |
| 4 | Documentation | Emphasize zero-config usage | None |

**Estimated Effort**: 0.5-1 day

---

## Implementation Priority Matrix

| Feature | Effort | Impact | ROI | Priority |
|---------|--------|--------|-----|----------|
| **Secret Detection** | 2-3 days | High | Security differentiator | **P1** |
| **CI/CD Validation** | 1-2 days | Medium | Enterprise feature | **P2** |
| **Zero-Config UX** | 0.5-1 day | Medium | Quick win, low effort | **P3** |
| **Style Framework Detection** | 3-5 days | Low | Niche, React-only | **P4** |

**Total Estimated Effort**: 7-11 days to close all gaps

---

## Strategic Recommendations

### 1. Messaging Strategy

**Don't say**: "We're better than LogicStamp"

**Do say**:
> "LogicStamp is excellent for React UI context. Semfora analyzes your **entire** codebase - 26 languages, every call path, every duplicate, every risk factor - with sub-second incremental updates."

### 2. Feature Prioritization

1. **Secret Detection (P1)**: High visibility, easy to implement, security is marketable
2. **CI Validation (P2)**: Enterprise teams need this, infrastructure already exists
3. **Zero-Config (P3)**: Low effort, improves onboarding
4. **Style Frameworks (P4)**: Only if targeting React-heavy market

### 3. Performance Benchmarks

Create public benchmarks comparing:
- 100-file React project (LogicStamp's sweet spot)
- 1000-file polyglot monorepo (Semfora's advantage)
- Incremental update speed (LogicStamp: full rescan, Semfora: <500ms)

### 4. Differentiation to Emphasize

| Semfora Exclusive | LogicStamp Exclusive |
|-------------------|----------------------|
| 26 languages | Tailwind/MUI/Shadcn metadata |
| Call graphs (bidirectional) | Secret detection |
| Duplicate detection | CI/CD tooling |
| Risk scoring | React-specific contracts |
| MCP server (30+ tools) | - |
| Real-time daemon | - |
| 4-layer index | - |
| BM25 semantic search | - |

---

## Conclusion

**LogicStamp Context** is a well-executed niche tool targeting React developers who want quick AI context. It excels at:
- Style framework awareness (Tailwind, MUI, etc.)
- Security scanning
- Zero-config simplicity
- CI/CD integration

**Semfora Engine** is a comprehensive semantic analysis platform with:
- 13x more language support (26 vs 2)
- Deep analysis (call graphs, duplicates, risk)
- Superior performance (incremental updates, caching)
- Native AI integration (MCP server)

**Competitive Threat Level**: Low. The tools serve different markets:
- LogicStamp → Individual React developers
- Semfora → Enterprise teams with polyglot codebases

**Feature gaps are closable** in 7-11 days of effort, but the fundamental architecture differences mean Semfora will always have deeper capabilities.
