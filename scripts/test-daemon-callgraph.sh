#!/bin/bash
# Test that daemon file watcher triggers call graph regeneration
#
# This verifies the fix for call graph not updating after file saves

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ENGINE_BIN="$PROJECT_DIR/target/release/semfora-engine"
DAEMON_BIN="$PROJECT_DIR/target/release/semfora-daemon"

# Use a small test repo
TEST_REPO="${1:-/home/kadajett/Dev/semfora-test-repos/repos/express-examples}"

echo "==================================="
echo "  Call Graph Regeneration Test"
echo "==================================="
echo ""
echo "Repo: $TEST_REPO"
echo ""

# Verify binaries exist
if [ ! -f "$ENGINE_BIN" ]; then
    echo "Error: Engine binary not found. Run 'cargo build --release' first."
    exit 1
fi

# Find a TypeScript or JavaScript file to modify
TEST_FILE=$(find "$TEST_REPO" -name "*.ts" -o -name "*.js" 2>/dev/null | grep -v node_modules | head -1)
if [ -z "$TEST_FILE" ]; then
    echo "Error: No TypeScript/JavaScript files found in $TEST_REPO"
    exit 1
fi

echo "Test file: $TEST_FILE"
echo ""

# Generate unique function name
TIMESTAMP=$(date +%s)
FUNC_NAME="testCallGraphRegen_${TIMESTAMP}"

echo "1. Creating initial index..."
"$ENGINE_BIN" --shard --format toon --dir "$TEST_REPO" > /dev/null 2>&1
echo "   Done."

echo ""
echo "2. Getting initial call graph..."
INITIAL_CALL_GRAPH=$("$ENGINE_BIN" --get-call-graph --dir "$TEST_REPO" 2>/dev/null | wc -l)
echo "   Initial call graph lines: $INITIAL_CALL_GRAPH"

echo ""
echo "3. Adding test function to file..."
# Save original content
ORIGINAL_CONTENT=$(cat "$TEST_FILE")

# Add a function that makes a call (for call graph)
cat >> "$TEST_FILE" << EOF

// Test function for call graph regeneration test
export function ${FUNC_NAME}() {
    console.log("test");
    return process.env.NODE_ENV;
}
EOF

echo "   Added function: $FUNC_NAME"

echo ""
echo "4. Running incremental reindex (simulating what daemon does)..."

# The daemon uses LayerSynchronizer with cache, let's simulate this
# by running the shard command which should update graphs
"$ENGINE_BIN" --shard --format toon --dir "$TEST_REPO" > /dev/null 2>&1
echo "   Done."

echo ""
echo "5. Searching for new symbol..."
SEARCH_RESULT=$("$ENGINE_BIN" --search-symbols "$FUNC_NAME" --dir "$TEST_REPO" 2>/dev/null)
if echo "$SEARCH_RESULT" | grep -q "$FUNC_NAME"; then
    echo "   PASS: Symbol found in index"
else
    echo "   FAIL: Symbol NOT found in index"
fi

echo ""
echo "6. Getting symbol hash from index..."
# Find cache directory for this repo
CACHE_DIR=$(ls -d ~/.cache/semfora/*/ 2>/dev/null | head -1)
if [ -n "$CACHE_DIR" ]; then
    SYMBOL_ENTRY=$(grep "$FUNC_NAME" "${CACHE_DIR}symbol_index.jsonl" 2>/dev/null | head -1)
    # Extract hash from {"s":"name","h":"hash",...} format
    HASH=$(echo "$SYMBOL_ENTRY" | sed 's/.*"h":"\([^"]*\)".*/\1/')
    echo "   Symbol entry: $SYMBOL_ENTRY"
    echo "   Hash: $HASH"
else
    echo "   WARNING: Could not find cache directory"
    HASH=""
fi

echo ""
echo "7. Checking call graph for new symbol..."
CALL_GRAPH=$("$ENGINE_BIN" --get-call-graph --dir "$TEST_REPO" 2>/dev/null)
ENTRY_COUNT=$(echo "$CALL_GRAPH" | tr ',' '\n' | grep -c ':' || echo "0")
echo "   Call graph entries: $ENTRY_COUNT"

# Check if our function's hash appears in the call graph
if [ -n "$HASH" ] && echo "$CALL_GRAPH" | grep -q "$HASH"; then
    echo "   PASS: Symbol hash found in call graph!"
    echo ""
    echo "   Call graph entry:"
    echo "$CALL_GRAPH" | tr ',' '\n' | grep "$HASH"
else
    echo "   MISS: Symbol hash NOT in call graph"
    if [ -z "$HASH" ]; then
        echo "   (Hash was not extracted from symbol index)"
    fi
fi

echo ""
echo "8. Restoring original file..."
echo "$ORIGINAL_CONTENT" > "$TEST_FILE"
echo "   Done."

echo ""
echo "==================================="
echo "  Test Complete"
echo "==================================="
