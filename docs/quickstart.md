# Quick Start Guide

Get up and running with Semfora Engine in 5 minutes.

## Installation

```bash
# Clone the repository
git clone https://github.com/Semfora-org/semfora-engine.git
cd semfora-engine

# Build release binaries
cargo build --release

# (Optional) Add to PATH
export PATH="$PATH:$(pwd)/target/release"
```

The build produces three binaries in `target/release/`:

| Binary | Purpose |
|--------|---------|
| `semfora-engine` | CLI for semantic analysis, indexing, and querying |
| `semfora-engine-server` | MCP server for AI agent integration |
| `semfora-daemon` | WebSocket daemon for real-time updates |

## CLI Usage

### Step 1: Index a Repository

Navigate to any git repository and create an index:

```bash
cd /path/to/your/project

# Generate sharded index
semfora-engine --dir . --shard
```

This creates a semantic index in `~/.cache/semfora/` with:
- Repository overview
- Per-module symbol data
- Call graph relationships
- Symbol lookup index

### Step 2: Search for Code

Once indexed, you can search:

```bash
# Search for symbols by name
semfora-engine --search-symbols "authenticate"

# Filter by symbol type
semfora-engine --search-symbols "handle" --kind fn

# Filter by risk level
semfora-engine --search-symbols "process" --risk high

# List symbols in a specific module
semfora-engine --list-symbols api
```

### Step 3: Get Repository Overview

```bash
# High-level architecture summary
semfora-engine --get-overview

# List all modules
semfora-engine --list-modules

# Get call graph
semfora-engine --get-call-graph
```

### Step 4: Analyze Changes

```bash
# Analyze uncommitted changes
semfora-engine --uncommitted

# Diff against main branch
semfora-engine --diff main

# Analyze a specific file
semfora-engine path/to/file.rs
```

## MCP Server for AI Agents

### Setting Up with Claude Code

1. Add to your Claude Code MCP configuration (`~/.config/claude/claude_desktop_config.json`):

```json
{
  "mcpServers": {
    "semfora-engine": {
      "type": "stdio",
      "command": "/path/to/semfora-engine/target/release/semfora-engine-server",
      "args": ["--repo", "/path/to/your/project"],
      "env": {
        "RUST_LOG": "semfora_engine=info"
      }
    }
  }
}
```

2. Restart Claude Code. The AI will now have access to semantic code analysis tools.

### MCP Server Options

```bash
# Start server for current directory
semfora-engine-server

# Start server for a specific repository
semfora-engine-server --repo /path/to/project
```

### Available MCP Tools

Once connected, the AI has access to:

| Tool | Description |
|------|-------------|
| `generate_index` | Create/update semantic index |
| `get_repo_overview` | Get architecture summary |
| `search_symbols` | Find symbols by name |
| `get_symbol` | Get detailed symbol info |
| `get_symbol_source` | Get source code for a symbol |
| `analyze_diff` | Analyze git changes |
| `run_tests` | Run project tests |

## WebSocket Daemon (Advanced)

For real-time index updates and multi-client support:

```bash
# Start the daemon
semfora-daemon --port 9847

# Connect via WebSocket client and send:
# {"type": "connect", "directory": "/path/to/project"}
```

See [WebSocket Daemon](websocket-daemon.md) for full protocol documentation.

## Common Workflows

### Code Review

```bash
# 1. Index the repository
semfora-engine --dir . --shard

# 2. Analyze the PR diff
semfora-engine --diff origin/main

# 3. Find high-risk changes
semfora-engine --search-symbols "*" --risk high
```

### Codebase Exploration

```bash
# 1. Get overview
semfora-engine --get-overview

# 2. List modules
semfora-engine --list-modules

# 3. Explore a specific module
semfora-engine --list-symbols components

# 4. Get details on a symbol
semfora-engine --get-symbol <hash>
```

### Incremental Updates

```bash
# Initial full index
semfora-engine --dir . --shard

# Later: incremental update (only changed files)
semfora-engine --dir . --shard --incremental
```

## Troubleshooting

### "No index found"

Run `semfora-engine --dir . --shard` to create an index first.

### Stale index

Run `semfora-engine --dir . --shard --incremental` to update.

### View cache info

```bash
semfora-engine --cache-info
```

### Clear cache

```bash
semfora-engine --cache-clear
```

## Next Steps

- [CLI Reference](cli.md) - Full command documentation
- [Features](features.md) - Incremental indexing, layered indexes, risk assessment
- [Adding Languages](adding-languages.md) - Extend language support
