# Context Optimization Architecture

## Overview

This document describes the context gathering optimization system implemented in the Semfora ADK. The system achieves 70%+ token reduction through intelligent routing, persistent state, and query-driven tool selection.

## Architecture Components

```
┌─────────────────────────────────────────────────────────────────┐
│                    ADK Session State                             │
│  temp:current_symbols    user:repo_overviews   app:index_cache  │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              SemforaOrchestrator (Model B)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ ToolRouter  │  │  Keywords   │  │  Context    │              │
│  │ (decision   │  │  Extractor  │  │  Budget     │              │
│  │  tree)      │  │  (enhanced) │  │  (priority) │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
                              │
┌─────────────────────────────────────────────────────────────────┐
│              Semfora Engine (Rust MCP Server)                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │
│  │ Batch Tools │  │  Session    │  │  Staleness  │              │
│  │ get_symbols │  │  Tracking   │  │  Auto-fix   │              │
│  └─────────────┘  └─────────────┘  └─────────────┘              │
└─────────────────────────────────────────────────────────────────┘
```

## Token Budget Reference

| Tool | Token Cost | Use Case |
|------|-----------|----------|
| `search_symbols` | ~400 | Find symbols by name |
| `list_symbols` | ~800 | Browse module contents |
| `get_symbol` | ~350 | Get detailed semantic info |
| `get_symbol_source` | ~400 | Get actual source code |
| `get_repo_overview` | ~300 | Understand architecture |
| `get_module` | 8,000-12,000 | **AVOID** - Use query-driven pattern |

## Core Components

### 1. SemforaState (state.py)

ADK-compatible state management with prefix-based scoping:

```python
from semfora_adk import SemforaState

state = SemforaState()

# Temp state (cleared after task)
state.set_temp("current_symbols", ["hash1", "hash2"])

# Session state (persists during conversation)
state.set_session("viewed_symbols", ["hash1"])

# User state (persists across sessions)
state.set_user("repo_overview:/path/to/repo", overview_data)

# App state (shared across users)
state.set_app("index_cache_version", "1.0.0")
```

#### State Prefix Mapping

| Prefix | Scope | Lifecycle | Use Case |
|--------|-------|-----------|----------|
| `temp:` | Invocation | Single task | Current task symbols, routing decisions |
| (none) | Session | Conversation | Viewed symbols, search history |
| `user:` | User | Cross-session | Repo overviews by path, preferences |
| `app:` | Application | Global | Index cache metadata, shared configs |

### 2. ToolRouter (router.py)

Deterministic decision tree for optimal tool selection. **No AI calls** - pure pattern matching.

```python
from semfora_adk import ToolRouter

router = ToolRouter(available_modules={"api", "components", "lib"})
decision = router.route("Fix the handleLogin function in auth/login.ts:45")

print(decision.route_type)      # RouteType.DIRECT_FILE
print(decision.tool_sequence)   # ["get_symbol_source"]
print(decision.estimated_tokens) # 400
```

#### Decision Tree

```
1. File path mentioned? → get_symbol_source (~400 tokens)
2. Symbol name (CamelCase/snake_case)? → search_symbols → get_symbol (~750 tokens)
3. Module inferred from keywords? → list_symbols → get_symbol (~1,150 tokens)
4. Generic keywords? → search_symbols batch → get_symbol (~1,500 tokens)
5. Fallback → repo_overview only (~300 tokens)

NEVER: get_module (8-12k tokens)
```

#### Route Types

| Route Type | When Used | Token Cost | Example Task |
|------------|-----------|------------|--------------|
| `DIRECT_FILE` | File path detected | ~400 | "Fix src/auth/login.ts:45" |
| `SYMBOL_SEARCH` | Symbol name detected | ~750 | "Update the handleLogin function" |
| `MODULE_BROWSE` | Module inferred | ~1,150 | "Show me the API endpoints" |
| `KEYWORD_SEARCH` | Generic keywords | ~1,500 | "How does authentication work?" |
| `OVERVIEW_ONLY` | No actionable refs | ~300 | "What is this codebase?" |

### 3. KeywordExtractor (keywords.py)

Domain-aware keyword extraction for routing decisions:

```python
from semfora_adk import KeywordExtractor

extractor = KeywordExtractor()
result = extractor.extract("Fix the handleLogin error handling in auth service")

print(result.symbol_candidates)  # ["handleLogin"]
print(result.compound_terms)     # ["error_handling"]
print(result.inferred_modules)   # ["api", "services"]
print(result.get_search_terms(limit=5))  # Top search terms
```

#### Keyword Categories

| Category | Examples | Use Case |
|----------|----------|----------|
| `SYMBOL_NAME` | CamelCase, snake_case | Direct symbol lookup |
| `CODE_CONCEPT` | async, callback, closure | Pattern matching |
| `OPERATION` | create, delete, update | Action detection |
| `ARCHITECTURE` | controller, service, middleware | Module inference |
| `ERROR` | exception, crash, bug | Debug context |
| `DATA` | model, schema, entity | Data flow analysis |
| `UI` | component, button, modal | UI module inference |
| `TEST` | test, spec, mock | Test context |

## Optimized Context Gathering

### Before (Inefficient)

```python
# Old approach: ~20,500 tokens per task
async def _gather_context_for_task(self, task):
    keywords = self._extract_keywords(task)  # Basic extraction

    # Expensive: multiple search calls
    for keyword in keywords[:3]:
        results = await self.tools.search_symbols(keyword)  # 400 tokens each

    # Expensive: fetch full module
    module = await self.tools.get_module("api")  # 10,000 tokens!

    # Sequential symbol fetches
    for symbol in results[:5]:
        detail = await self.tools.get_symbol(symbol.hash)  # 350 each
```

### After (Optimized)

```python
# New approach: ~5,450 tokens per task (73% reduction)
async def _gather_context_for_task(self, task):
    # Start invocation tracking
    self.state.start_invocation()

    # Smart routing determines optimal tool sequence
    decision = self.router.route(task)  # 0 tokens, pure logic

    # Execute based on route type
    if decision.route_type == RouteType.DIRECT_FILE:
        # Direct file access: ~400 tokens
        context = await self._execute_file_route(decision)

    elif decision.route_type == RouteType.SYMBOL_SEARCH:
        # Symbol lookup: ~750 tokens
        context = await self._execute_symbol_route(decision)

    elif decision.route_type == RouteType.MODULE_BROWSE:
        # Module browse (using list_symbols, NOT get_module): ~1,150 tokens
        context = await self._execute_module_route(decision)

    # Track usage
    self.state.record_fetch("context_gather", decision.estimated_tokens)

    # End invocation (moves symbols to session history)
    self.state.end_invocation()
```

## Token Savings Analysis

| Scenario | Before | After | Savings |
|----------|--------|-------|---------|
| Typical task | 20,500 | 5,450 | 73% |
| Symbol lookup | 8,350 | 1,150 | 86% |
| Module exploration | 10,800 | 2,600 | 76% |
| File reference | 8,700 | 800 | 91% |

## Best Practices

### 1. Always Use Query-Driven Pattern

```python
# GOOD: Use list_symbols + get_symbol
symbols = await tools.list_symbols(module="api", limit=50)  # 800 tokens
detail = await tools.get_symbol(symbols[0].hash)  # 350 tokens
# Total: 1,150 tokens

# BAD: Use get_module
module = await tools.get_module("api")  # 10,000+ tokens!
```

### 2. Check Cache Before Fetching

```python
# Check state before fetching
if not self.state.should_fetch_symbol(symbol_hash):
    # Already have it in current task
    return cached_content

# Check memory cache
cached = self.memory.get_symbol(symbol_hash)
if cached and cached.content:
    return cached.content

# Only fetch if not cached
detail = await tools.get_symbol(symbol_hash)
```

### 3. Use Batch Operations

```python
# GOOD: Batch search
results = await tools.search_symbols_batch(
    ["login", "auth", "user"],
    limit_per_query=10
)  # Single analysis, multiple filters

# BAD: Sequential searches
for keyword in keywords:
    results = await tools.search_symbols(keyword)  # Multiple analyses
```

### 4. Track Token Usage

```python
# Record all fetches for debugging
state.record_fetch("search_symbols", {"query": "login"}, 400)

# Get session usage summary
usage = state.get_session_token_usage()
total = state.get_total_tokens_used()

print(f"Token usage: {usage}")  # {"search_symbols": 800, "get_symbol": 1050}
print(f"Total: {total}")        # 1850
```

## Engine Batch Operations

### get_symbols (Batch Symbol Fetch)

Fetch up to 20 symbols in a single call:

```json
{
  "hashes": ["abc123", "def456", "ghi789"],
  "include_source": true,
  "context": 5
}
```

### check_index (Staleness Check)

Check if index is stale and optionally auto-refresh:

```json
{
  "auto_refresh": true,
  "max_age": 3600
}
```

Response:
```json
{
  "is_stale": true,
  "age_seconds": 7200,
  "modified_files": ["src/auth/login.ts", "src/api/users.ts"],
  "refreshed": true
}
```

## Integration with ADK

### Session State Integration

```python
# The SemforaState class is designed to integrate with ADK session.state
# For ADK integration, wrap it in an adapter:

class ADKStateAdapter:
    def __init__(self, adk_session):
        self.session = adk_session

    def get(self, key: str):
        return self.session.state.get(key)

    def set(self, key: str, value: Any):
        self.session.state[key] = value

# Usage with ADK
state = SemforaState(storage=ADKStateAdapter(adk_session))
```

### Memory Service Integration

For cross-session persistence, implement a `PersistentStateStorage`:

```python
class PersistentStateStorage:
    def __init__(self, memory_service):
        self.memory = memory_service

    async def get(self, key: str):
        return await self.memory.recall(key)

    async def set(self, key: str, value: Any):
        await self.memory.remember(key, value)
```

## Debugging

### Get Routing Decision

```python
# The routing decision is stored in temp state
decision = state.get_routing_decision()
print(f"Route: {decision['route_type']}")
print(f"Tools: {decision['tool_sequence']}")
print(f"Tokens: {decision['estimated_tokens']}")
print(f"Reasoning: {decision['reasoning']}")
```

### Get Context Summary

```python
summary = state.get_context_summary()
print(json.dumps(summary, indent=2))
# {
#   "temp": {"current_symbols_count": 3, "has_routing_decision": true},
#   "session": {"viewed_symbols": 15, "search_history": 5, "total_tokens": 4500},
#   "user": {"cached_repos": 2}
# }
```

### Get Orchestrator Status

```python
status = orchestrator.get_status()
print(f"Available modules: {status['available_modules']}")
print(f"State: {status['state']}")
print(f"Memory: {status['memory']}")
```
