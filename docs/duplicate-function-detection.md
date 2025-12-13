# Duplicate Function Detection Architecture

**Date:** December 12, 2025

## Overview

This document describes the architecture for detecting duplicate and near-duplicate functions across codebases using semantic analysis. The feature leverages existing call graph and function shard data to identify similar functions without requiring a vector database, while maintaining sub-5ms query performance.

## Problem Statement

AI agents (and humans) often accidentally duplicate code. Functions that perform the same operation get written multiple times across a codebase, leading to:

- Maintenance burden (fixes applied inconsistently)
- Bugs fixed in one place but not another
- Inconsistent behavior across the application
- Bloated codebase size

## Design Principles

1. **No Vector DB** - Must work with existing semantic data structures
2. **Millisecond Performance** - Sub-5ms even for massive codebases (50K+ functions)
3. **Smart Boilerplate Detection** - Ignore common patterns that should be duplicated
4. **High Precision** - 90%+ match threshold to avoid false positives

## Available Data

From existing function shards, each symbol already has:

- Symbol name
- Calls made (deduplicated, with async/try context)
- State changes
- Control flow patterns
- Dependencies
- Risk level
- Line count and location

This semantic data serves as a **fingerprint** - no source code comparison needed.

## Architecture

### 1. Signature Generation (Index Time)

Generate a lightweight signature for each function during indexing:

```rust
struct FunctionSignature {
    /// Symbol hash for lookup
    hash: SymbolHash,

    /// Name tokens for BM25-style matching
    /// e.g., "handleUserLogin" -> ["handle", "user", "login"]
    name_tokens: Vec<String>,

    /// Structural fingerprints (64-bit hashes for fast comparison)
    call_fingerprint: u64,           // Hash of sorted, filtered call names
    control_flow_fingerprint: u64,   // Hash of control flow pattern sequence
    state_fingerprint: u64,          // Hash of state mutation patterns

    /// Semantic data for fine-grained matching
    business_calls: HashSet<String>, // Calls minus common utilities
    param_count: u8,

    /// Boilerplate classification (pre-computed)
    boilerplate_category: Option<BoilerplateCategory>,
}
```

### 2. Boilerplate Detection

Functions are classified as "expected duplicates" based on patterns. These are excluded from duplicate detection by default.

```rust
enum BoilerplateCategory {
    /// React Query hooks (useQuery/useMutation with minimal logic)
    ReactQuery,

    /// React hook wrappers (useState/useEffect patterns)
    ReactHook,

    /// Event handlers (handleClick, onChange with 1-2 calls)
    EventHandler,

    /// API route handlers (Express/Next.js patterns)
    ApiRoute,

    /// Test setup functions (beforeEach, setup, teardown)
    TestSetup,

    /// Config/export boilerplate (module.exports patterns)
    ConfigExport,

    /// Type guard functions (isX() type checking)
    TypeGuard,
}
```

#### Classification Heuristics

```rust
fn classify_boilerplate(summary: &SymbolSummary) -> Option<BoilerplateCategory> {
    // React Query: uses query hooks with minimal other logic
    let query_calls = summary.calls.iter()
        .filter(|c| matches!(c.name.as_str(),
            "useQuery" | "useMutation" | "useQueryClient" | "useSuspenseQuery"))
        .count();
    if query_calls > 0 && summary.calls.len() <= query_calls + 2 {
        return Some(BoilerplateCategory::ReactQuery);
    }

    // Event handler: starts with handle/on, minimal calls
    if (summary.name.starts_with("handle") || summary.name.starts_with("on"))
        && summary.calls.len() <= 2
    {
        return Some(BoilerplateCategory::EventHandler);
    }

    // Type guard: isX pattern with single type check
    if summary.name.starts_with("is")
        && summary.control_flow.len() <= 1
        && summary.calls.is_empty()
    {
        return Some(BoilerplateCategory::TypeGuard);
    }

    // Test setup
    if matches!(summary.name.as_str(),
        "beforeEach" | "afterEach" | "beforeAll" | "afterAll" | "setup" | "teardown"
    ) {
        return Some(BoilerplateCategory::TestSetup);
    }

    None
}
```

### 3. Two-Phase Similarity Matching

#### Phase A: Coarse Filter

O(n) scan with early exit conditions to filter candidates quickly:

```rust
fn coarse_filter(target: &FunctionSignature, all: &[FunctionSignature]) -> Vec<usize> {
    all.iter().enumerate()
        .filter(|(_, candidate)| {
            // Skip self
            if candidate.hash == target.hash { return false; }

            // Skip boilerplate (unless target is same category)
            if let Some(cat) = &candidate.boilerplate_category {
                if target.boilerplate_category.as_ref() != Some(cat) {
                    return false;
                }
            }

            // Structural quick-checks (early exit)
            if candidate.param_count.abs_diff(target.param_count) > 2 {
                return false;
            }
            if candidate.business_calls.len().abs_diff(target.business_calls.len()) > 3 {
                return false;
            }

            // Fingerprint hamming distance (bit differences)
            let call_dist = (target.call_fingerprint ^ candidate.call_fingerprint).count_ones();
            if call_dist > 12 { return false; }

            true
        })
        .map(|(i, _)| i)
        .collect()
}
```

This typically filters to <5% of total functions.

#### Phase B: Fine-Grained Similarity

Compute detailed similarity only for filtered candidates:

```rust
fn compute_similarity(a: &SymbolSummary, b: &SymbolSummary) -> f64 {
    // Filter out common utilities
    let a_calls = filter_utilities(&a.calls);
    let b_calls = filter_utilities(&b.calls);

    // Weighted similarity components
    let call_similarity = jaccard(&a_calls, &b_calls);
    let name_similarity = token_jaccard(&a.name, &b.name);
    let control_similarity = control_flow_similarity(&a.control_flow, &b.control_flow);
    let state_similarity = state_change_similarity(&a.state_changes, &b.state_changes);

    // Weights tuned for business logic detection
    call_similarity * 0.45 +      // Most important: what functions do
    name_similarity * 0.20 +      // Names often indicate purpose
    control_similarity * 0.20 +   // Similar branching = similar logic
    state_similarity * 0.15       // Similar mutations = similar behavior
}
```

#### Utility Function Filtering

Common utility calls are excluded from similarity calculation:

```rust
fn is_utility(name: &str) -> bool {
    matches!(name,
        // Console/logging
        "console.log" | "console.error" | "console.warn" | "console.info" |
        // JSON operations
        "JSON.stringify" | "JSON.parse" |
        // Type conversions
        "toString" | "parseInt" | "parseFloat" | "String" | "Number" | "Boolean" |
        // Array methods (too common)
        "map" | "filter" | "reduce" | "forEach" | "find" | "some" | "every" |
        "push" | "pop" | "shift" | "unshift" | "slice" | "splice" |
        // Object utilities
        "Object.keys" | "Object.values" | "Object.entries" | "Object.assign" |
        "Array.from" | "Array.isArray" |
        // Promise utilities
        "Promise.resolve" | "Promise.reject" | "Promise.all"
    )
}
```

### 4. Performance Budget

| Operation | Complexity | 50K Functions |
|-----------|------------|---------------|
| Coarse filter | O(n) | ~500μs |
| Fine similarity | O(k), k ≈ 0.05n | ~2ms |
| Clustering | O(k²) | ~200μs |
| **Total** | | **~3ms** |

For smaller codebases (1K functions like zod): <100μs

### 5. Result Structures

```rust
/// A group of similar functions
struct DuplicateCluster {
    /// The "canonical" version (typically longest/most documented)
    primary: SymbolRef,

    /// Other functions similar to primary
    duplicates: Vec<DuplicateMatch>,

    /// Human-readable summary
    summary: String,  // e.g., "3 functions implement user validation"
}

struct DuplicateMatch {
    /// Reference to the duplicate symbol
    symbol: SymbolRef,

    /// Similarity score (0.0 - 1.0)
    similarity: f64,

    /// Specific differences found
    differences: Vec<Difference>,
}

enum Difference {
    /// Function has an extra call not in primary
    ExtraCall(String),

    /// Function is missing a call from primary
    MissingCall(String),

    /// Different control flow structure
    DifferentControlFlow(String),

    /// Different state mutations
    DifferentStateMutation(String),
}
```

### 6. Smart Insights

Beyond raw duplicate detection, generate actionable insights:

```rust
enum DuplicateInsight {
    /// Exact semantic match
    /// "fetchUser in api/users.ts and getUser in services/user.ts are identical"
    ExactDuplicate {
        a: SymbolRef,
        b: SymbolRef
    },

    /// Pattern repeated many times
    /// "handleSubmit appears 5 times - consider extracting to shared hook"
    RepeatedPattern {
        name: String,
        count: usize,
        locations: Vec<SymbolRef>
    },

    /// Same base function with divergent implementations
    /// "validateUser has 3 versions with slight differences - may cause bugs"
    DivergentDuplicates {
        base_name: String,
        variants: Vec<(SymbolRef, Vec<Difference>)>
    },

    /// Duplicate that's acceptable (boilerplate)
    /// "useUserQuery is correctly repeated (React Query pattern)"
    AcceptableDuplicate {
        symbol: SymbolRef,
        reason: BoilerplateCategory
    },
}
```

### 7. API Design

#### MCP Tools

```rust
/// Find all duplicate clusters in repository
#[mcp_tool]
pub fn find_duplicates(
    /// Minimum similarity threshold (default: 0.90)
    threshold: Option<f64>,

    /// Whether to exclude boilerplate patterns (default: true)
    exclude_boilerplate: Option<bool>,

    /// Filter to specific module
    module: Option<String>,

    /// Repository path
    path: Option<String>,
) -> Result<Vec<DuplicateCluster>>

/// Check if a specific function has duplicates
#[mcp_tool]
pub fn check_duplicates(
    /// Symbol hash to check
    symbol_hash: String,

    /// Minimum similarity threshold (default: 0.90)
    threshold: Option<f64>,

    /// Repository path
    path: Option<String>,
) -> Result<Vec<DuplicateMatch>>
```

### 8. Integration Points

#### Real-Time Checking (WebSocket Layer)

When the working layer updates, automatically check new/modified functions:

```rust
// In working layer update handler
async fn on_symbol_changed(symbol: &SymbolSummary) {
    if let Some(duplicates) = check_duplicates_realtime(symbol, 0.90) {
        emit_event(WorkspaceEvent::PotentialDuplicate {
            new_function: symbol.clone(),
            similar_to: duplicates,
        }).await;
    }
}
```

#### AI Agent Integration

Before an AI agent creates a new function, check for existing similar code:

```
Agent: "I'll create a function validateUserInput..."
Engine: "Similar function exists: validateInput in src/utils/validation.ts (92% match)"
Agent: "I'll use the existing validateInput function instead."
```

### 9. Memory Overhead

Per function signature: ~200 bytes

| Field | Size |
|-------|------|
| name_tokens | ~40 bytes avg |
| fingerprints (3x u64) | 24 bytes |
| business_calls HashSet | ~80 bytes avg |
| boilerplate_category | 8 bytes |
| other fields | ~48 bytes |

**Total for 50K functions: ~10MB additional memory**

## Implementation Phases

### Phase 1: Core Infrastructure
- Add `FunctionSignature` struct to schema
- Implement signature generation during extraction
- Add fingerprint computation utilities

### Phase 2: Boilerplate Detection
- Implement `BoilerplateCategory` enum
- Add classification heuristics for each category
- Integrate into signature generation

### Phase 3: Similarity Matching
- Implement coarse filter with early exit
- Implement fine-grained similarity computation
- Add clustering algorithm for grouping duplicates

### Phase 4: MCP Integration
- Add `find_duplicates` tool
- Add `check_duplicates` tool
- Add result formatting and insights generation

### Phase 5: Real-Time Integration
- Add WebSocket event for duplicate detection
- Integrate with working layer updates
- Add configurable thresholds

### Phase 6: Testing & Benchmarks
- Add unit tests for each component
- Add integration tests with real codebases
- Add criterion benchmarks to verify <5ms performance

## Future Enhancements

1. **Cross-Repository Detection** - Find duplicates across multiple repos in a workspace
2. **Historical Tracking** - Track when duplicates were introduced (git blame integration)
3. **Auto-Refactoring Suggestions** - Generate refactoring plans to consolidate duplicates
4. **Custom Boilerplate Rules** - Allow users to define project-specific boilerplate patterns
5. **Similarity Explanation** - Detailed breakdown of why two functions are considered similar
