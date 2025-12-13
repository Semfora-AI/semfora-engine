#!/bin/bash
# Performance Testing Suite for Semfora Engine
#
# This script runs comprehensive performance tests including:
# - Criterion benchmarks (indexing, queries, incremental)
# - Multi-repo daemon stress tests
# - Memory profiling
# - CPU profiling
#
# Usage: ./scripts/perf-test.sh [options]
#   --quick       Run only quick benchmarks
#   --full        Run all benchmarks including large repos
#   --daemon      Run daemon stress tests only
#   --memory      Run memory profiling only
#   --report      Generate HTML reports

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
RESULTS_DIR="$PROJECT_DIR/target/perf-results"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Test repos directory
REPOS_DIR="${SEMFORA_TEST_REPOS:-/home/kadajett/Dev/semfora-test-repos/repos}"

# Parse arguments
QUICK_MODE=false
FULL_MODE=false
DAEMON_ONLY=false
MEMORY_ONLY=false
GENERATE_REPORT=false

while [[ $# -gt 0 ]]; do
    case $1 in
        --quick) QUICK_MODE=true; shift ;;
        --full) FULL_MODE=true; shift ;;
        --daemon) DAEMON_ONLY=true; shift ;;
        --memory) MEMORY_ONLY=true; shift ;;
        --report) GENERATE_REPORT=true; shift ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
done

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Semfora Engine Performance Tests${NC}"
echo -e "${BLUE}  Timestamp: $TIMESTAMP${NC}"
echo -e "${BLUE}========================================${NC}"

# Create results directory
mkdir -p "$RESULTS_DIR"

# Check for required binaries
check_binaries() {
    echo -e "\n${YELLOW}Checking required binaries...${NC}"

    if ! command -v cargo &> /dev/null; then
        echo -e "${RED}Error: cargo not found${NC}"
        exit 1
    fi

    # Build release binaries
    echo "Building release binaries..."
    cd "$PROJECT_DIR"
    cargo build --release

    echo -e "${GREEN}Binaries built successfully${NC}"
}

# Run criterion benchmarks
run_criterion_benchmarks() {
    echo -e "\n${YELLOW}Running Criterion Benchmarks...${NC}"
    cd "$PROJECT_DIR"

    if [ "$QUICK_MODE" = true ]; then
        echo "Running quick benchmarks (indexing small repos only)..."
        cargo bench --bench indexing -- --sample-size 5 "small"
    else
        echo "Running indexing benchmarks..."
        cargo bench --bench indexing 2>&1 | tee "$RESULTS_DIR/indexing_$TIMESTAMP.log"

        echo "Running query benchmarks..."
        cargo bench --bench queries 2>&1 | tee "$RESULTS_DIR/queries_$TIMESTAMP.log"

        echo "Running incremental benchmarks..."
        cargo bench --bench incremental 2>&1 | tee "$RESULTS_DIR/incremental_$TIMESTAMP.log"
    fi

    echo -e "${GREEN}Criterion benchmarks completed${NC}"
}

# Multi-repo daemon stress test
run_daemon_stress_test() {
    echo -e "\n${YELLOW}Running Daemon Stress Test...${NC}"

    DAEMON_BIN="$PROJECT_DIR/target/release/semfora-daemon"
    ENGINE_BIN="$PROJECT_DIR/target/release/semfora-engine"
    DAEMON_LOG="$RESULTS_DIR/daemon_stress_$TIMESTAMP.log"

    # Initialize log file
    echo "=== Daemon Stress Test Log - $TIMESTAMP ===" > "$DAEMON_LOG"
    echo "Started at: $(date)" >> "$DAEMON_LOG"

    if [ ! -f "$DAEMON_BIN" ]; then
        echo -e "${RED}Error: daemon binary not found. Run 'cargo build --release' first.${NC}"
        return 1
    fi

    # Find test repos
    REPOS=()
    for repo in "$REPOS_DIR"/*; do
        if [ -d "$repo" ]; then
            REPOS+=("$repo")
        fi
    done

    NUM_REPOS=${#REPOS[@]}
    echo "Found $NUM_REPOS test repositories"

    if [ $NUM_REPOS -lt 5 ]; then
        echo -e "${YELLOW}Warning: Less than 5 repos found. Using available repos.${NC}"
    fi

    # Use up to 10 repos for stress test
    TEST_REPOS=("${REPOS[@]:0:10}")

    # Clear any existing caches to ensure fresh benchmarks
    echo -e "\n${BLUE}Phase 0: Clearing existing caches${NC}"
    for repo in "${TEST_REPOS[@]}"; do
        "$ENGINE_BIN" --cache-clear --dir "$repo" 2>/dev/null || true
    done

    echo -e "\n${BLUE}Phase 1: Sequential indexing of ${#TEST_REPOS[@]} repos${NC}"
    START_TIME=$(date +%s.%N)

    for repo in "${TEST_REPOS[@]}"; do
        REPO_NAME=$(basename "$repo")
        echo "  Indexing: $REPO_NAME"
        "$ENGINE_BIN" --dir "$repo" --format toon 2>/dev/null || true
    done

    END_TIME=$(date +%s.%N)
    SEQ_DURATION=$(echo "$END_TIME - $START_TIME" | bc)
    echo -e "${GREEN}Sequential indexing completed in ${SEQ_DURATION}s${NC}"
    echo "Phase 1 - Sequential indexing: ${SEQ_DURATION}s (${#TEST_REPOS[@]} repos)" >> "$DAEMON_LOG"

    # Clear caches again for parallel test
    for repo in "${TEST_REPOS[@]}"; do
        "$ENGINE_BIN" --cache-clear --dir "$repo" 2>/dev/null || true
    done

    echo -e "\n${BLUE}Phase 2: Parallel indexing of ${#TEST_REPOS[@]} repos${NC}"
    START_TIME=$(date +%s.%N)

    PIDS=()
    for repo in "${TEST_REPOS[@]}"; do
        "$ENGINE_BIN" --dir "$repo" --format toon 2>/dev/null &
        PIDS+=($!)
    done

    # Wait for all
    for pid in "${PIDS[@]}"; do
        wait $pid 2>/dev/null || true
    done

    END_TIME=$(date +%s.%N)
    PAR_DURATION=$(echo "$END_TIME - $START_TIME" | bc)
    echo -e "${GREEN}Parallel indexing completed in ${PAR_DURATION}s${NC}"

    SPEEDUP=$(echo "scale=2; $SEQ_DURATION / $PAR_DURATION" | bc)
    echo -e "${GREEN}Speedup: ${SPEEDUP}x${NC}"
    echo "Phase 2 - Parallel indexing: ${PAR_DURATION}s (speedup: ${SPEEDUP}x)" >> "$DAEMON_LOG"

    echo -e "\n${BLUE}Phase 3: Daemon startup with multiple repos${NC}"

    # Start daemon
    echo "Starting daemon..."
    "$DAEMON_BIN" &
    DAEMON_PID=$!
    sleep 2

    # Check if daemon is running
    if kill -0 $DAEMON_PID 2>/dev/null; then
        echo -e "${GREEN}Daemon started (PID: $DAEMON_PID)${NC}"
    else
        echo -e "${RED}Failed to start daemon${NC}"
        return 1
    fi

    echo -e "\n${BLUE}Phase 4: Query stress test${NC}"

    # Run multiple queries in parallel
    echo "Running 100 parallel search queries..."
    START_TIME=$(date +%s.%N)

    QUERY_PIDS=()
    for i in {1..100}; do
        (
            # Pick a random repo
            REPO="${TEST_REPOS[$((RANDOM % ${#TEST_REPOS[@]}))]}"
            "$ENGINE_BIN" --search-symbols "function" --dir "$REPO" >/dev/null 2>&1 || true
        ) &
        QUERY_PIDS+=($!)
    done

    # Wait only for query processes, not the daemon
    for pid in "${QUERY_PIDS[@]}"; do
        wait $pid 2>/dev/null || true
    done

    END_TIME=$(date +%s.%N)
    QUERY_DURATION=$(echo "$END_TIME - $START_TIME" | bc)
    QPS=$(echo "scale=2; 100 / $QUERY_DURATION" | bc)
    echo -e "${GREEN}100 queries completed in ${QUERY_DURATION}s (${QPS} QPS)${NC}"

    # Clean up daemon
    echo -e "\nStopping daemon..."
    kill $DAEMON_PID 2>/dev/null || true
    wait $DAEMON_PID 2>/dev/null || true

    # Write results
    cat > "$RESULTS_DIR/daemon_stress_$TIMESTAMP.json" <<EOF
{
    "timestamp": "$TIMESTAMP",
    "num_repos": ${#TEST_REPOS[@]},
    "sequential_indexing_seconds": $SEQ_DURATION,
    "parallel_indexing_seconds": $PAR_DURATION,
    "speedup": $SPEEDUP,
    "queries": 100,
    "query_duration_seconds": $QUERY_DURATION,
    "queries_per_second": $QPS
}
EOF

    echo -e "${GREEN}Daemon stress test completed${NC}"
}

# Memory profiling
run_memory_profiling() {
    echo -e "\n${YELLOW}Running Memory Profiling...${NC}"

    ENGINE_BIN="$PROJECT_DIR/target/release/semfora-engine"

    # Find a medium-sized repo
    MEDIUM_REPO=""
    for repo in "$REPOS_DIR"/*; do
        if [ -d "$repo" ]; then
            # Count files
            FILE_COUNT=$(find "$repo" -type f -name "*.ts" -o -name "*.tsx" -o -name "*.js" 2>/dev/null | wc -l)
            if [ $FILE_COUNT -gt 50 ] && [ $FILE_COUNT -lt 500 ]; then
                MEDIUM_REPO="$repo"
                break
            fi
        fi
    done

    if [ -z "$MEDIUM_REPO" ]; then
        echo "No suitable medium repo found, using first available"
        MEDIUM_REPO=$(ls -d "$REPOS_DIR"/*/ 2>/dev/null | head -1)
    fi

    if [ -z "$MEDIUM_REPO" ]; then
        echo -e "${RED}No test repos found${NC}"
        return 1
    fi

    REPO_NAME=$(basename "$MEDIUM_REPO")
    echo "Profiling memory usage on: $REPO_NAME"

    # Clear cache before profiling to ensure fresh index
    echo "Clearing cache for fresh benchmark..."
    "$ENGINE_BIN" --cache-clear --dir "$MEDIUM_REPO" 2>/dev/null || true

    # Check if we have memory profiling tools
    if command -v /usr/bin/time &> /dev/null; then
        echo -e "\n${BLUE}Indexing memory usage:${NC}"
        /usr/bin/time -v "$ENGINE_BIN" --dir "$MEDIUM_REPO" --format toon 2>&1 | grep -E "(Maximum resident|User time|System time|Elapsed)" | tee -a "$RESULTS_DIR/memory_$TIMESTAMP.log"

        echo -e "\n${BLUE}Query memory usage (search):${NC}"
        /usr/bin/time -v "$ENGINE_BIN" --search-symbols "function" --dir "$MEDIUM_REPO" 2>&1 | grep -E "(Maximum resident|User time|System time|Elapsed)" | tee -a "$RESULTS_DIR/memory_$TIMESTAMP.log"

        echo -e "\n${BLUE}Query memory usage (overview):${NC}"
        /usr/bin/time -v "$ENGINE_BIN" --get-overview --dir "$MEDIUM_REPO" 2>&1 | grep -E "(Maximum resident|User time|System time|Elapsed)" | tee -a "$RESULTS_DIR/memory_$TIMESTAMP.log"
    else
        echo "Using basic timing (install GNU time for detailed memory stats)"

        echo -e "\n${BLUE}Indexing:${NC}"
        "$ENGINE_BIN" --cache-clear --dir "$MEDIUM_REPO" 2>/dev/null || true
        time "$ENGINE_BIN" --dir "$MEDIUM_REPO" --format toon

        echo -e "\n${BLUE}Search query:${NC}"
        time "$ENGINE_BIN" --search-symbols "function" --dir "$MEDIUM_REPO"
    fi

    echo -e "${GREEN}Memory profiling completed${NC}"
}

# Generate summary report
generate_report() {
    echo -e "\n${YELLOW}Generating Summary Report...${NC}"

    REPORT_FILE="$RESULTS_DIR/report_$TIMESTAMP.md"

    cat > "$REPORT_FILE" <<EOF
# Semfora Engine Performance Report

**Date:** $(date)
**Commit:** $(cd "$PROJECT_DIR" && git rev-parse --short HEAD 2>/dev/null || echo "N/A")

## System Information

- **OS:** $(uname -s) $(uname -r)
- **CPU:** $(grep -m1 "model name" /proc/cpuinfo 2>/dev/null | cut -d: -f2 | xargs || sysctl -n machdep.cpu.brand_string 2>/dev/null || echo "N/A")
- **Memory:** $(free -h 2>/dev/null | grep Mem | awk '{print $2}' || sysctl -n hw.memsize 2>/dev/null | awk '{print $1/1024/1024/1024 " GB"}' || echo "N/A")

## Benchmark Results

### Criterion Benchmarks

See detailed reports in \`target/criterion/\`

EOF

    # Append daemon stress test results if available
    LATEST_DAEMON=$(ls -t "$RESULTS_DIR"/daemon_stress_*.json 2>/dev/null | head -1)
    if [ -n "$LATEST_DAEMON" ]; then
        cat >> "$REPORT_FILE" <<EOF
### Daemon Stress Test

\`\`\`json
$(cat "$LATEST_DAEMON")
\`\`\`

EOF
    fi

    # Append memory profiling results if available
    LATEST_MEMORY=$(ls -t "$RESULTS_DIR"/memory_*.log 2>/dev/null | head -1)
    if [ -n "$LATEST_MEMORY" ]; then
        cat >> "$REPORT_FILE" <<EOF
### Memory Profiling

\`\`\`
$(cat "$LATEST_MEMORY")
\`\`\`

EOF
    fi

    echo -e "${GREEN}Report generated: $REPORT_FILE${NC}"
}

# Main execution
main() {
    check_binaries

    if [ "$DAEMON_ONLY" = true ]; then
        run_daemon_stress_test
    elif [ "$MEMORY_ONLY" = true ]; then
        run_memory_profiling
    else
        run_criterion_benchmarks
        run_daemon_stress_test
        run_memory_profiling
    fi

    if [ "$GENERATE_REPORT" = true ] || [ "$DAEMON_ONLY" = false ] && [ "$MEMORY_ONLY" = false ]; then
        generate_report
    fi

    echo -e "\n${GREEN}========================================${NC}"
    echo -e "${GREEN}  Performance tests completed!${NC}"
    echo -e "${GREEN}  Results saved to: $RESULTS_DIR${NC}"
    echo -e "${GREEN}========================================${NC}"
}

main
