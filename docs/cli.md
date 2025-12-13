# Semfora CLI Reference

The `semfora-engine` CLI is a semantic code analyzer that produces compressed TOON output for AI-assisted code review.

## Quick Start

```bash
# Build
cargo build --release

# Index a project
semfora-engine --dir /path/to/project --shard

# Search for symbols
semfora-engine --search-symbols "authenticate"

# Analyze uncommitted changes
semfora-engine --uncommitted
```

## Installation

```bash
cargo build --release
# Binary: target/release/semfora-engine
```

## Basic Usage

```bash
# Analyze a single file
semfora-engine path/to/file.rs

# Analyze a directory (recursive)
semfora-engine --dir path/to/project

# Analyze uncommitted changes
semfora-engine --uncommitted

# Diff against main branch
semfora-engine --diff
```

## Operation Modes

### Single File Analysis

```bash
semfora-engine path/to/file.rs
semfora-engine path/to/file.ts --format json
```

### Directory Analysis

```bash
# Analyze all files in a directory
semfora-engine --dir ./src

# Limit recursion depth (default: 10)
semfora-engine --dir ./src --max-depth 5

# Filter by file extension
semfora-engine --dir ./src --ext rs --ext ts

# Include test files (excluded by default)
semfora-engine --dir ./src --allow-tests

# Summary statistics only
semfora-engine --dir ./src --summary-only
```

### Git Diff Analysis

```bash
# Diff against auto-detected base branch (main/master)
semfora-engine --diff

# Diff against a specific branch
semfora-engine --diff develop

# Explicit base branch
semfora-engine --diff --base origin/main

# Analyze uncommitted changes (working directory vs HEAD)
semfora-engine --uncommitted

# Analyze a specific commit
semfora-engine --commit abc123

# Analyze all commits since base branch
semfora-engine --commits
```

## Output Formats

```bash
# TOON format (default) - token-efficient for AI consumption
semfora-engine file.rs --format toon

# JSON format - standard structured output
semfora-engine file.rs --format json

# Verbose output with AST info
semfora-engine file.rs --verbose

# Print parsed AST (debugging)
semfora-engine file.rs --print-ast
```

## Sharded Indexing

For large repositories, create a sharded index for fast querying:

### Generate Index

```bash
# Generate sharded index (writes to ~/.cache/semfora-engine/)
semfora-engine --dir . --shard

# Incremental indexing (only re-index changed files)
semfora-engine --dir . --shard --incremental

# Filter extensions during indexing
semfora-engine --dir . --shard --ext ts --ext tsx
```

### Query Index

```bash
# Get repository overview
semfora-engine --get-overview

# List all modules in the index
semfora-engine --list-modules

# Get a specific module's symbols
semfora-engine --get-module api

# Search for symbols by name
semfora-engine --search-symbols "login"

# List all symbols in a module
semfora-engine --list-symbols auth

# Get a specific symbol by hash
semfora-engine --get-symbol abc123def456

# Get the call graph
semfora-engine --get-call-graph
```

### Query Filtering

```bash
# Filter by symbol kind
semfora-engine --search-symbols "handle" --kind fn

# Filter by risk level
semfora-engine --list-symbols api --risk high

# Limit results (default: 50)
semfora-engine --search-symbols "test" --limit 20
```

## Cache Management

```bash
# Show cache information
semfora-engine --cache-info

# Clear cache for current directory
semfora-engine --cache-clear

# Prune caches older than N days
semfora-engine --cache-prune 30
```

## Static Analysis

```bash
# Run static code analysis on the index
semfora-engine --analyze

# Analyze a specific module only
semfora-engine --analyze --analyze-module api
```

## Token Analysis

Analyze token efficiency of TOON compression:

```bash
# Full detailed report
semfora-engine file.rs --analyze-tokens full

# Compact single-line summary
semfora-engine file.rs --analyze-tokens compact

# Include compact JSON comparison
semfora-engine file.rs --analyze-tokens full --compare-compact
```

## Benchmarking

```bash
# Run token efficiency benchmark
semfora-engine --benchmark
```

## Test File Exclusion

By default, test files are excluded from analysis. Test patterns by language:

| Language | Excluded Patterns |
|----------|-------------------|
| Rust | `*_test.rs`, `tests/**` |
| TypeScript/JS | `*.test.ts`, `*.spec.ts`, `__tests__/**` |
| Python | `test_*.py`, `*_test.py`, `tests/**` |
| Go | `*_test.go` |
| Java | `*Test.java`, `*Tests.java` |

Use `--allow-tests` to include test files.

## Directory for Index Queries

When using query commands (`--get-overview`, `--search-symbols`, etc.), the CLI uses the cache for the current working directory. The cache location is determined by the git remote URL hash for reproducibility.

## Examples

### Typical Workflow

```bash
# 1. Generate index for a project
cd my-project
semfora-engine --dir . --shard

# 2. Get project overview
semfora-engine --get-overview

# 3. Search for specific functionality
semfora-engine --search-symbols "authenticate" --kind fn

# 4. Get details on a symbol
semfora-engine --get-symbol abc123def456

# 5. Analyze changes before commit
semfora-engine --uncommitted

# 6. Analyze feature branch diff
semfora-engine --diff main
```

### Code Review Workflow

```bash
# Analyze PR changes
semfora-engine --diff origin/main

# Focus on specific file types
semfora-engine --diff origin/main --ext ts --ext tsx

# Get summary only
semfora-engine --diff origin/main --summary-only
```

## Environment Variables

- `RUST_LOG`: Control logging verbosity (e.g., `RUST_LOG=semfora_engine=debug`)

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | File not found or IO error |
| 2 | Unsupported language |
| 3 | Parse failure |
| 4 | Semantic extraction or query error |
| 5 | Git error (not a git repo, etc.) |

## See Also

- [Features](features.md) - Incremental indexing, layered indexes, risk assessment
- [WebSocket Daemon](websocket-daemon.md) - Real-time index updates via WebSocket
- [Main README](../README.md) - Supported languages and architecture
