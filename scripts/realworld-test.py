#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""
Real-World Behavior Tests for Semfora Engine

Tests the reactive indexing system by simulating real developer workflows
and measuring how quickly changes are reflected in symbols AND call graph.

Test scenarios:
1. File save → symbol + call graph detection (via incremental reindex)
2. Git checkout → branch detection
3. Git pull simulation → base layer update
4. Multiple file changes → batch detection

Metrics tracked:
- Time from change to symbol detection
- Time from change to call graph detection
- Poll count before detection
- Success/failure rates

Usage:
    uv run scripts/realworld-test.py                        # Run all tests on default repos
    uv run scripts/realworld-test.py --repo /path/to/repo   # Test specific repo
    uv run scripts/realworld-test.py --file-save-only       # Just file save tests
    uv run scripts/realworld-test.py --git-ops-only         # Just git operation tests
    uv run scripts/realworld-test.py --iterations 10        # Run each test N times

Note: File-save detection via daemon file watcher is faster than CLI-based testing.
      This script uses CLI commands to simulate the indexing path.
"""

import argparse
import json
import os
import random
import shutil
import string
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field, asdict
from datetime import datetime
from pathlib import Path
from typing import Optional
import statistics

# ============================================================================
# Configuration
# ============================================================================

SCRIPT_DIR = Path(__file__).parent.resolve()
PROJECT_DIR = SCRIPT_DIR.parent
ENGINE_BIN = PROJECT_DIR / "target" / "release" / "semfora-engine"
RESULTS_DIR = PROJECT_DIR / "target" / "realworld-results"

# Default test repos
DEFAULT_REPOS = [
    Path("/home/kadajett/Dev/semfora-test-repos/repos/zod"),
    Path("/home/kadajett/Dev/semfora-test-repos/repos/express-examples"),
    Path("/home/kadajett/Dev/semfora-test-repos/repos/next.js"),
]

# Polling configuration
POLL_INTERVAL_MS = 50  # Check every 50ms
MAX_POLL_TIME_MS = 30000  # Give up after 30 seconds
SETTLE_TIME_MS = 100  # Wait before starting to poll

# ============================================================================
# Data Structures
# ============================================================================

@dataclass
class DetectionResult:
    """Result of a single detection test"""
    test_name: str
    repo_name: str
    operation: str  # file_save, git_checkout, git_pull, etc.

    # Timing
    total_time_ms: float
    symbol_detection_ms: Optional[float] = None
    call_graph_detection_ms: Optional[float] = None

    # Status
    symbol_detected: bool = False
    call_graph_detected: bool = False
    success: bool = False

    # Details
    poll_count: int = 0
    error: Optional[str] = None
    metadata: dict = field(default_factory=dict)

@dataclass
class TestSummary:
    """Summary of multiple test runs"""
    test_name: str
    runs: int
    successes: int
    failures: int

    # Timing stats (ms)
    avg_symbol_detection_ms: float = 0
    min_symbol_detection_ms: float = 0
    max_symbol_detection_ms: float = 0

    avg_call_graph_detection_ms: float = 0
    min_call_graph_detection_ms: float = 0
    max_call_graph_detection_ms: float = 0

    # Call graph specific
    call_graph_success_rate: float = 0

# ============================================================================
# Utilities
# ============================================================================

def generate_unique_id() -> str:
    """Generate a unique identifier for test artifacts"""
    timestamp = int(time.time() * 1000)
    rand = ''.join(random.choices(string.ascii_lowercase, k=4))
    return f"test_{timestamp}_{rand}"

def run_cmd(cmd: list[str], cwd: Optional[Path] = None, timeout: int = 60) -> tuple[bool, str, str]:
    """Run command and return (success, stdout, stderr)"""
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=timeout,
            cwd=cwd
        )
        return result.returncode == 0, result.stdout, result.stderr
    except subprocess.TimeoutExpired:
        return False, "", "Timeout"
    except Exception as e:
        return False, "", str(e)

def engine_cmd(args: list[str], repo_path: Path, timeout: int = 120) -> tuple[bool, str]:
    """Run semfora-engine command"""
    cmd = [str(ENGINE_BIN)] + args + ["--dir", str(repo_path)]
    success, stdout, stderr = run_cmd(cmd, timeout=timeout)
    return success, stdout + stderr

def print_status(msg: str, color: str = "blue"):
    """Print colored status message"""
    colors = {
        "blue": "\033[94m",
        "green": "\033[92m",
        "yellow": "\033[93m",
        "red": "\033[91m",
        "cyan": "\033[96m"
    }
    reset = "\033[0m"
    print(f"{colors.get(color, '')}{msg}{reset}")

def ensure_index(repo_path: Path) -> bool:
    """Ensure repo has an index, create if missing"""
    success, output = engine_cmd(["--cache-info"], repo_path)
    if "No cache" in output or not success:
        print_status(f"  Creating index for {repo_path.name}...", "yellow")
        success, _ = engine_cmd(["--shard", "--format", "toon"], repo_path, timeout=300)
        return success
    return True

# ============================================================================
# Detection Functions
# ============================================================================

def search_symbol(repo_path: Path, symbol_name: str) -> tuple[bool, Optional[str]]:
    """Search for a symbol by name, return (found, hash)"""
    success, output = engine_cmd(["--search-symbols", symbol_name, "--limit", "50"], repo_path)
    if not success:
        return False, None

    # Parse output to find our symbol
    # Output format varies, look for the symbol name
    if symbol_name in output:
        # Try to extract hash from output
        for line in output.split('\n'):
            if symbol_name in line:
                # Look for hash pattern (hex string)
                parts = line.split()
                for part in parts:
                    if len(part) == 16 and all(c in '0123456789abcdef' for c in part):
                        return True, part
        return True, None  # Found but couldn't extract hash
    return False, None

def check_call_graph_for_symbol(repo_path: Path, symbol_hash: Optional[str], symbol_name: str) -> bool:
    """Check if symbol appears in call graph (as caller or callee)"""
    success, output = engine_cmd(["--get-call-graph"], repo_path)
    if not success:
        return False

    # If we have the hash, look for it directly
    if symbol_hash and symbol_hash in output:
        return True

    # Otherwise, the call graph contains hashes, not names
    # We can't directly search by name in the call graph
    # But if the symbol exists and has calls, it should be there
    return symbol_hash is not None and symbol_hash in output

def poll_for_detection(
    repo_path: Path,
    symbol_name: str,
    check_call_graph: bool = True,
    max_time_ms: int = MAX_POLL_TIME_MS
) -> DetectionResult:
    """Poll until symbol (and optionally call graph) is detected"""
    start_time = time.perf_counter()
    poll_count = 0
    symbol_detected = False
    symbol_time = None
    call_graph_detected = False
    call_graph_time = None
    symbol_hash = None

    # Initial settle time
    time.sleep(SETTLE_TIME_MS / 1000)

    while True:
        elapsed_ms = (time.perf_counter() - start_time) * 1000

        if elapsed_ms > max_time_ms:
            break

        poll_count += 1

        # Check symbol
        if not symbol_detected:
            found, hash_val = search_symbol(repo_path, symbol_name)
            if found:
                symbol_detected = True
                symbol_time = elapsed_ms
                symbol_hash = hash_val

        # Check call graph (only if symbol detected and we have hash)
        if check_call_graph and symbol_detected and not call_graph_detected:
            if check_call_graph_for_symbol(repo_path, symbol_hash, symbol_name):
                call_graph_detected = True
                call_graph_time = elapsed_ms

        # Success condition
        if symbol_detected and (not check_call_graph or call_graph_detected):
            break

        time.sleep(POLL_INTERVAL_MS / 1000)

    total_time = (time.perf_counter() - start_time) * 1000

    return DetectionResult(
        test_name="poll_detection",
        repo_name=repo_path.name,
        operation="poll",
        total_time_ms=total_time,
        symbol_detection_ms=symbol_time,
        call_graph_detection_ms=call_graph_time,
        symbol_detected=symbol_detected,
        call_graph_detected=call_graph_detected,
        success=symbol_detected,  # Base success on symbol detection
        poll_count=poll_count,
        metadata={"symbol_name": symbol_name, "symbol_hash": symbol_hash}
    )

# ============================================================================
# Test Scenarios
# ============================================================================

def test_file_save_detection(repo_path: Path, iterations: int = 3) -> list[DetectionResult]:
    """
    Test: Add a new function to a file, trigger reindex, measure detection time

    This simulates what the file watcher would do, but via CLI.
    In production, the daemon's file watcher handles this automatically.
    """
    results = []

    # Find a suitable TypeScript/JavaScript file to modify
    def is_valid_file(f: Path) -> bool:
        s = str(f)
        name = f.name.lower()
        # Exclude node_modules, test files, type definitions
        if "node_modules" in s:
            return False
        if ".d.ts" in name:
            return False
        if name.endswith(".test.ts") or name.endswith(".test.tsx") or name.endswith(".spec.ts"):
            return False
        if "/__tests__/" in s or "/tests/" in s:
            return False
        return True

    test_files = list(repo_path.glob("**/*.ts")) + list(repo_path.glob("**/*.tsx"))
    test_files = [f for f in test_files if is_valid_file(f)]

    # If no TS files, try JS
    if not test_files:
        test_files = list(repo_path.glob("**/*.js")) + list(repo_path.glob("**/*.jsx"))
        test_files = [f for f in test_files if is_valid_file(f)]

    if not test_files:
        print_status(f"  No suitable test files found in {repo_path.name}", "yellow")
        return results

    test_file = test_files[0]

    for i in range(iterations):
        unique_id = generate_unique_id()
        func_name = f"semforaTestFunc_{unique_id}"

        # Read original content
        original_content = test_file.read_text()

        # Create test function that calls something (for call graph)
        test_code = f'''
// Semfora test function - will be removed
export function {func_name}() {{
    console.log("test");
    return Date.now();
}}
'''

        try:
            # Add test function to file
            test_file.write_text(original_content + test_code)

            # Record start time
            start_time = time.perf_counter()

            # Trigger reindex (simulates what file watcher would do)
            # Using full reindex since --incremental needs commits
            success, output = engine_cmd(["--shard", "--format", "toon"], repo_path, timeout=300)

            reindex_time = (time.perf_counter() - start_time) * 1000

            if not success:
                results.append(DetectionResult(
                    test_name="file_save_detection",
                    repo_name=repo_path.name,
                    operation="file_save",
                    total_time_ms=reindex_time,
                    success=False,
                    error="Reindex failed"
                ))
                continue

            # Now poll for detection
            detection = poll_for_detection(repo_path, func_name, check_call_graph=True)

            # Combine times
            total_time = reindex_time + detection.total_time_ms

            results.append(DetectionResult(
                test_name="file_save_detection",
                repo_name=repo_path.name,
                operation="file_save",
                total_time_ms=total_time,
                symbol_detection_ms=reindex_time + (detection.symbol_detection_ms or 0),
                call_graph_detection_ms=reindex_time + (detection.call_graph_detection_ms or 0) if detection.call_graph_detected else None,
                symbol_detected=detection.symbol_detected,
                call_graph_detected=detection.call_graph_detected,
                success=detection.symbol_detected,
                poll_count=detection.poll_count,
                metadata={
                    "func_name": func_name,
                    "reindex_time_ms": reindex_time,
                    "file": str(test_file.relative_to(repo_path)),
                    "iteration": i + 1
                }
            ))

        finally:
            # Restore original file
            test_file.write_text(original_content)

    return results

def test_incremental_commit_detection(repo_path: Path, iterations: int = 3) -> list[DetectionResult]:
    """
    Test: Make a commit, run incremental reindex, measure detection time

    This tests the --incremental path which uses git SHA comparison.
    """
    results = []

    # Check if repo is a git repo
    if not (repo_path / ".git").exists():
        print_status(f"  {repo_path.name} is not a git repo, skipping commit test", "yellow")
        return results

    # Find a suitable file
    test_files = list(repo_path.glob("**/*.ts")) + list(repo_path.glob("**/*.tsx"))
    test_files = [f for f in test_files if "node_modules" not in str(f)]

    if not test_files:
        return results

    test_file = test_files[0]

    for i in range(iterations):
        unique_id = generate_unique_id()
        func_name = f"semforaCommitTest_{unique_id}"

        original_content = test_file.read_text()
        original_head = None

        try:
            # Get current HEAD
            success, stdout, _ = run_cmd(["git", "rev-parse", "HEAD"], cwd=repo_path)
            if success:
                original_head = stdout.strip()

            # Add test function
            test_code = f'\nexport function {func_name}() {{ return 42; }}\n'
            test_file.write_text(original_content + test_code)

            # Stage and commit
            run_cmd(["git", "add", str(test_file)], cwd=repo_path)
            run_cmd(["git", "commit", "-m", f"Semfora test commit {unique_id}"], cwd=repo_path)

            # Record start time
            start_time = time.perf_counter()

            # Run incremental reindex
            success, output = engine_cmd(["--shard", "--incremental", "--format", "toon"], repo_path, timeout=120)

            reindex_time = (time.perf_counter() - start_time) * 1000

            if not success:
                results.append(DetectionResult(
                    test_name="incremental_commit",
                    repo_name=repo_path.name,
                    operation="git_commit",
                    total_time_ms=reindex_time,
                    success=False,
                    error="Incremental reindex failed"
                ))
                continue

            # Poll for detection
            detection = poll_for_detection(repo_path, func_name, check_call_graph=True)

            total_time = reindex_time + detection.total_time_ms

            results.append(DetectionResult(
                test_name="incremental_commit",
                repo_name=repo_path.name,
                operation="git_commit",
                total_time_ms=total_time,
                symbol_detection_ms=reindex_time + (detection.symbol_detection_ms or 0),
                call_graph_detection_ms=reindex_time + (detection.call_graph_detection_ms or 0) if detection.call_graph_detected else None,
                symbol_detected=detection.symbol_detected,
                call_graph_detected=detection.call_graph_detected,
                success=detection.symbol_detected,
                poll_count=detection.poll_count,
                metadata={
                    "func_name": func_name,
                    "reindex_time_ms": reindex_time,
                    "iteration": i + 1
                }
            ))

        finally:
            # Restore: reset to original HEAD
            if original_head:
                run_cmd(["git", "reset", "--hard", original_head], cwd=repo_path)
            else:
                # Fallback: just restore file and amend
                test_file.write_text(original_content)
                run_cmd(["git", "checkout", "--", str(test_file)], cwd=repo_path)

    return results

def test_git_checkout_detection(repo_path: Path) -> list[DetectionResult]:
    """
    Test: Check out a different branch, measure time to detect branch change

    Note: This requires the repo to have multiple branches.
    """
    results = []

    if not (repo_path / ".git").exists():
        return results

    # Get current branch
    success, stdout, _ = run_cmd(["git", "branch", "--show-current"], cwd=repo_path)
    if not success:
        return results

    current_branch = stdout.strip()

    # Get list of branches
    success, stdout, _ = run_cmd(["git", "branch", "-a"], cwd=repo_path)
    if not success:
        return results

    branches = [b.strip().replace("* ", "") for b in stdout.split('\n') if b.strip()]
    other_branches = [b for b in branches if b != current_branch and not b.startswith("remotes/")]

    if not other_branches:
        print_status(f"  No other branches in {repo_path.name}, skipping checkout test", "yellow")
        return results

    target_branch = other_branches[0]

    try:
        # Checkout other branch
        start_time = time.perf_counter()

        success, _, _ = run_cmd(["git", "checkout", target_branch], cwd=repo_path)
        if not success:
            return results

        checkout_time = (time.perf_counter() - start_time) * 1000

        # Trigger reindex for new branch
        success, _ = engine_cmd(["--shard", "--format", "toon"], repo_path, timeout=300)

        reindex_time = (time.perf_counter() - start_time) * 1000

        results.append(DetectionResult(
            test_name="git_checkout",
            repo_name=repo_path.name,
            operation="git_checkout",
            total_time_ms=reindex_time,
            symbol_detected=success,
            success=success,
            metadata={
                "from_branch": current_branch,
                "to_branch": target_branch,
                "checkout_time_ms": checkout_time
            }
        ))

    finally:
        # Return to original branch
        run_cmd(["git", "checkout", current_branch], cwd=repo_path)

    return results

def test_multi_file_changes(repo_path: Path, num_files: int = 5) -> list[DetectionResult]:
    """
    Test: Change multiple files simultaneously, measure batch detection time
    """
    results = []

    # Find test files
    test_files = list(repo_path.glob("**/*.ts")) + list(repo_path.glob("**/*.tsx"))
    test_files = [f for f in test_files if "node_modules" not in str(f)][:num_files]

    if len(test_files) < 2:
        return results

    unique_id = generate_unique_id()
    func_names = [f"semforaMulti_{unique_id}_{i}" for i in range(len(test_files))]
    original_contents = {}

    try:
        # Modify all files
        for i, (test_file, func_name) in enumerate(zip(test_files, func_names)):
            original_contents[test_file] = test_file.read_text()
            test_code = f'\nexport function {func_name}() {{ return {i}; }}\n'
            test_file.write_text(original_contents[test_file] + test_code)

        # Record start time
        start_time = time.perf_counter()

        # Trigger reindex
        success, _ = engine_cmd(["--shard", "--format", "toon"], repo_path, timeout=300)

        reindex_time = (time.perf_counter() - start_time) * 1000

        if not success:
            results.append(DetectionResult(
                test_name="multi_file_change",
                repo_name=repo_path.name,
                operation="multi_file_save",
                total_time_ms=reindex_time,
                success=False,
                error="Reindex failed"
            ))
            return results

        # Check how many functions were detected
        detected_count = 0
        for func_name in func_names:
            found, _ = search_symbol(repo_path, func_name)
            if found:
                detected_count += 1

        total_time = (time.perf_counter() - start_time) * 1000

        results.append(DetectionResult(
            test_name="multi_file_change",
            repo_name=repo_path.name,
            operation="multi_file_save",
            total_time_ms=total_time,
            symbol_detected=detected_count == len(func_names),
            success=detected_count == len(func_names),
            metadata={
                "files_changed": len(test_files),
                "symbols_detected": detected_count,
                "reindex_time_ms": reindex_time
            }
        ))

    finally:
        # Restore all files
        for test_file, content in original_contents.items():
            test_file.write_text(content)

    return results

# ============================================================================
# Test Runner
# ============================================================================

def summarize_results(results: list[DetectionResult], test_name: str) -> TestSummary:
    """Create summary statistics from test results"""
    successful = [r for r in results if r.success]

    symbol_times = [r.symbol_detection_ms for r in successful if r.symbol_detection_ms is not None]
    cg_times = [r.call_graph_detection_ms for r in successful if r.call_graph_detection_ms is not None]

    return TestSummary(
        test_name=test_name,
        runs=len(results),
        successes=len(successful),
        failures=len(results) - len(successful),
        avg_symbol_detection_ms=statistics.mean(symbol_times) if symbol_times else 0,
        min_symbol_detection_ms=min(symbol_times) if symbol_times else 0,
        max_symbol_detection_ms=max(symbol_times) if symbol_times else 0,
        avg_call_graph_detection_ms=statistics.mean(cg_times) if cg_times else 0,
        min_call_graph_detection_ms=min(cg_times) if cg_times else 0,
        max_call_graph_detection_ms=max(cg_times) if cg_times else 0,
        call_graph_success_rate=len(cg_times) / len(successful) if successful else 0
    )

def run_all_tests(repos: list[Path], iterations: int = 3, file_save_only: bool = False, git_ops_only: bool = False) -> dict:
    """Run all tests and return results"""
    all_results = []
    summaries = []

    for repo_path in repos:
        if not repo_path.exists():
            print_status(f"Repo not found: {repo_path}", "red")
            continue

        print_status(f"\n{'='*60}")
        print_status(f"  Testing: {repo_path.name}")
        print_status(f"{'='*60}")

        # Ensure index exists
        if not ensure_index(repo_path):
            print_status(f"  Failed to create index for {repo_path.name}", "red")
            continue

        # Run tests
        if not git_ops_only:
            print_status(f"\n  [1/4] File Save Detection ({iterations} iterations)...", "cyan")
            results = test_file_save_detection(repo_path, iterations)
            all_results.extend(results)
            for r in results:
                status = "OK" if r.success else "FAIL"
                cg_status = "OK" if r.call_graph_detected else "MISS"
                print(f"    {status}: symbol={r.symbol_detection_ms:.0f}ms, call_graph={cg_status} ({r.call_graph_detection_ms:.0f}ms)" if r.call_graph_detection_ms else f"    {status}: symbol={r.symbol_detection_ms:.0f}ms, call_graph={cg_status}")

            print_status(f"\n  [2/4] Multi-File Changes...", "cyan")
            results = test_multi_file_changes(repo_path)
            all_results.extend(results)
            for r in results:
                status = "OK" if r.success else "FAIL"
                print(f"    {status}: {r.metadata.get('files_changed', 0)} files, {r.total_time_ms:.0f}ms")

        if not file_save_only:
            print_status(f"\n  [3/4] Incremental Commit Detection ({iterations} iterations)...", "cyan")
            results = test_incremental_commit_detection(repo_path, iterations)
            all_results.extend(results)
            for r in results:
                status = "OK" if r.success else "FAIL"
                print(f"    {status}: {r.total_time_ms:.0f}ms total, reindex={r.metadata.get('reindex_time_ms', 0):.0f}ms")

            print_status(f"\n  [4/4] Git Checkout Detection...", "cyan")
            results = test_git_checkout_detection(repo_path)
            all_results.extend(results)
            for r in results:
                status = "OK" if r.success else "FAIL"
                print(f"    {status}: {r.metadata.get('from_branch', '?')} -> {r.metadata.get('to_branch', '?')}, {r.total_time_ms:.0f}ms")

    # Create summaries by test type
    test_types = set(r.test_name for r in all_results)
    for test_type in test_types:
        type_results = [r for r in all_results if r.test_name == test_type]
        summaries.append(summarize_results(type_results, test_type))

    return {
        "results": [asdict(r) for r in all_results],
        "summaries": [asdict(s) for s in summaries],
        "timestamp": datetime.now().isoformat(),
        "repos_tested": [str(r) for r in repos if r.exists()]
    }

def print_summary(report: dict):
    """Print human-readable summary"""
    print_status(f"\n{'='*60}")
    print_status("  SUMMARY")
    print_status(f"{'='*60}\n")

    for summary in report["summaries"]:
        print_status(f"  {summary['test_name']}", "cyan")
        print(f"    Runs: {summary['runs']} | Success: {summary['successes']} | Fail: {summary['failures']}")

        if summary['avg_symbol_detection_ms'] > 0:
            print(f"    Symbol Detection: avg={summary['avg_symbol_detection_ms']:.0f}ms, min={summary['min_symbol_detection_ms']:.0f}ms, max={summary['max_symbol_detection_ms']:.0f}ms")

        if summary['avg_call_graph_detection_ms'] > 0:
            print(f"    Call Graph:       avg={summary['avg_call_graph_detection_ms']:.0f}ms, min={summary['min_call_graph_detection_ms']:.0f}ms, max={summary['max_call_graph_detection_ms']:.0f}ms")
            print(f"    Call Graph Success Rate: {summary['call_graph_success_rate']*100:.0f}%")
        print()

# ============================================================================
# Main
# ============================================================================

def main():
    parser = argparse.ArgumentParser(description="Real-World Behavior Tests for Semfora Engine")
    parser.add_argument("--repo", type=Path, action="append", help="Test specific repo(s)")
    parser.add_argument("--iterations", type=int, default=3, help="Iterations per test")
    parser.add_argument("--file-save-only", action="store_true", help="Only run file save tests")
    parser.add_argument("--git-ops-only", action="store_true", help="Only run git operation tests")
    parser.add_argument("--output", type=Path, help="Output JSON file")
    parser.add_argument("--no-build", action="store_true", help="Skip cargo build")
    args = parser.parse_args()

    # Build if needed
    if not args.no_build:
        print_status("Building release binary...", "yellow")
        result = subprocess.run(
            ["cargo", "build", "--release"],
            cwd=PROJECT_DIR,
            capture_output=True
        )
        if result.returncode != 0:
            print_status("Build failed!", "red")
            sys.exit(1)

    if not ENGINE_BIN.exists():
        print_status(f"Engine binary not found: {ENGINE_BIN}", "red")
        sys.exit(1)

    # Select repos
    repos = args.repo if args.repo else [r for r in DEFAULT_REPOS if r.exists()]

    if not repos:
        print_status("No test repos found!", "red")
        sys.exit(1)

    print_status(f"\n{'='*60}")
    print_status("  REAL-WORLD BEHAVIOR TESTS")
    print_status(f"  Repos: {len(repos)}")
    print_status(f"  Iterations: {args.iterations}")
    print_status(f"{'='*60}")

    # Run tests
    report = run_all_tests(
        repos,
        iterations=args.iterations,
        file_save_only=args.file_save_only,
        git_ops_only=args.git_ops_only
    )

    # Print summary
    print_summary(report)

    # Save results
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    output_path = args.output or RESULTS_DIR / f"realworld_{timestamp}.json"

    with open(output_path, "w") as f:
        json.dump(report, f, indent=2)

    print_status(f"Results saved to: {output_path}", "green")

if __name__ == "__main__":
    main()
