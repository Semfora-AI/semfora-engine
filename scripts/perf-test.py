#!/usr/bin/env -S uv run --script
# /// script
# requires-python = ">=3.11"
# dependencies = []
# ///
"""
Semfora Engine Performance Test Suite

Parallel performance testing with standardized JSON output.
Output format is compatible with Google Benchmark JSON for tooling interoperability.

Usage (with UV - recommended):
    uv run scripts/perf-test.py                    # Run all tests
    uv run scripts/perf-test.py --quick            # Quick smoke test
    uv run scripts/perf-test.py --indexing-only    # Just indexing benchmarks
    uv run scripts/perf-test.py --queries-only     # Just query benchmarks
    uv run scripts/perf-test.py --report           # Generate HTML report

Alternative (direct execution if UV is in PATH):
    ./scripts/perf-test.py --quick
"""

import argparse
import json
import os
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from concurrent.futures import ProcessPoolExecutor, ThreadPoolExecutor, as_completed
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
RESULTS_DIR = PROJECT_DIR / "target" / "perf-results"
REPOS_DIR = Path(os.environ.get("SEMFORA_TEST_REPOS", "/home/kadajett/Dev/semfora-test-repos/repos"))

ENGINE_BIN = PROJECT_DIR / "target" / "release" / "semfora-engine"
DAEMON_BIN = PROJECT_DIR / "target" / "release" / "semfora-daemon"

# Repo categories by expected size
SMALL_REPOS = ["nestjs-starter", "react-realworld", "angular-realworld", "sample-hugo"]
MEDIUM_REPOS = ["express-examples", "fastify-examples", "koa-examples", "zod", "routing-controllers"]
LARGE_REPOS = ["next.js", "typescript-eslint", "babel", "puppeteer", "playwright", "nextjs-examples"]

# Query patterns for benchmarking
SEARCH_PATTERNS = ["function", "export", "handler", "error", "async", "render", "parse", "interface"]

# ============================================================================
# Data Structures (Google Benchmark compatible)
# ============================================================================

@dataclass
class BenchmarkResult:
    """Single benchmark result - Google Benchmark compatible"""
    name: str
    real_time: float  # seconds
    cpu_time: float = 0.0  # seconds (optional)
    iterations: int = 1
    time_unit: str = "s"
    # Extended fields
    items_per_second: float = 0.0
    bytes_per_second: float = 0.0
    memory_peak_mb: float = 0.0
    error: Optional[str] = None
    metadata: dict = field(default_factory=dict)

@dataclass
class BenchmarkContext:
    """System context - Google Benchmark compatible"""
    date: str
    host_name: str
    executable: str
    num_cpus: int
    mhz_per_cpu: int = 0
    cpu_scaling_enabled: bool = False
    caches: list = field(default_factory=list)
    # Extended fields
    os: str = ""
    os_version: str = ""
    memory_gb: float = 0.0
    git_commit: str = ""
    rust_version: str = ""

@dataclass
class BenchmarkReport:
    """Full report - Google Benchmark compatible structure"""
    context: BenchmarkContext
    benchmarks: list  # List of BenchmarkResult

    def to_dict(self):
        return {
            "context": asdict(self.context),
            "benchmarks": [asdict(b) for b in self.benchmarks]
        }

# ============================================================================
# Utilities
# ============================================================================

def get_system_context() -> BenchmarkContext:
    """Gather system information"""
    git_commit = ""
    try:
        git_commit = subprocess.check_output(
            ["git", "rev-parse", "--short", "HEAD"],
            cwd=PROJECT_DIR, stderr=subprocess.DEVNULL
        ).decode().strip()
    except:
        pass

    rust_version = ""
    try:
        rust_version = subprocess.check_output(
            ["rustc", "--version"], stderr=subprocess.DEVNULL
        ).decode().strip()
    except:
        pass

    memory_gb = 0.0
    try:
        with open("/proc/meminfo") as f:
            for line in f:
                if line.startswith("MemTotal:"):
                    memory_gb = int(line.split()[1]) / 1024 / 1024
                    break
    except:
        pass

    return BenchmarkContext(
        date=datetime.now().isoformat(),
        host_name=platform.node(),
        executable=str(ENGINE_BIN),
        num_cpus=os.cpu_count() or 1,
        os=platform.system(),
        os_version=platform.release(),
        memory_gb=round(memory_gb, 1),
        git_commit=git_commit,
        rust_version=rust_version,
    )

def find_repos(category: str = "all") -> list[tuple[str, Path]]:
    """Find available test repos"""
    if not REPOS_DIR.exists():
        return []

    if category == "small":
        names = SMALL_REPOS
    elif category == "medium":
        names = MEDIUM_REPOS
    elif category == "large":
        names = LARGE_REPOS
    else:
        names = SMALL_REPOS + MEDIUM_REPOS + LARGE_REPOS

    repos = []
    for name in names:
        path = REPOS_DIR / name
        if path.exists() and path.is_dir():
            repos.append((name, path))

    # Also add any other repos found
    if category == "all":
        known = set(SMALL_REPOS + MEDIUM_REPOS + LARGE_REPOS)
        for entry in REPOS_DIR.iterdir():
            if entry.is_dir() and entry.name not in known:
                repos.append((entry.name, entry))

    return repos

def count_source_files(path: Path) -> int:
    """Count source files in a directory"""
    extensions = {'.ts', '.tsx', '.js', '.jsx', '.rs', '.py', '.go', '.java', '.c', '.cpp', '.h', '.hpp'}
    count = 0
    try:
        for root, dirs, files in os.walk(path):
            # Skip common non-source directories
            dirs[:] = [d for d in dirs if d not in {'node_modules', '.git', 'target', '__pycache__', 'dist', 'build'}]
            for f in files:
                if Path(f).suffix.lower() in extensions:
                    count += 1
    except:
        pass
    return count

def clear_cache(repo_path: Path) -> bool:
    """Clear cache for a repo"""
    try:
        subprocess.run(
            [str(ENGINE_BIN), "--cache-clear", "--dir", str(repo_path)],
            capture_output=True, timeout=30
        )
        return True
    except:
        return False

def run_with_timing(cmd: list[str], timeout: int = 300) -> tuple[float, bool, str]:
    """Run command and return (duration_seconds, success, output)"""
    start = time.perf_counter()
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout)
        duration = time.perf_counter() - start
        return duration, result.returncode == 0, result.stdout + result.stderr
    except subprocess.TimeoutExpired:
        return timeout, False, "Timeout"
    except Exception as e:
        return time.perf_counter() - start, False, str(e)

def print_progress(msg: str, color: str = "blue"):
    """Print colored progress message"""
    colors = {"blue": "\033[94m", "green": "\033[92m", "yellow": "\033[93m", "red": "\033[91m"}
    reset = "\033[0m"
    print(f"{colors.get(color, '')}{msg}{reset}")

# ============================================================================
# Benchmark Functions
# ============================================================================

def benchmark_index_repo(args: tuple[str, Path, bool]) -> BenchmarkResult:
    """Index a single repo and measure performance"""
    name, path, clear_first = args

    if clear_first:
        clear_cache(path)

    file_count = count_source_files(path)

    # Create temp cache directory
    with tempfile.TemporaryDirectory() as tmpdir:
        cmd = [str(ENGINE_BIN), "--dir", str(path), "--format", "toon"]
        duration, success, output = run_with_timing(cmd)

        return BenchmarkResult(
            name=f"indexing/{name}",
            real_time=duration,
            iterations=1,
            items_per_second=file_count / duration if duration > 0 else 0,
            error=None if success else output[:200],
            metadata={"files": file_count, "repo": name}
        )

def benchmark_search(args: tuple[str, Path, str, int]) -> BenchmarkResult:
    """Run search query benchmark"""
    repo_name, repo_path, pattern, iterations = args

    times = []
    for _ in range(iterations):
        cmd = [str(ENGINE_BIN), "--search-symbols", pattern, "--dir", str(repo_path), "--limit", "20"]
        duration, success, _ = run_with_timing(cmd, timeout=30)
        if success:
            times.append(duration)

    if not times:
        return BenchmarkResult(
            name=f"search/{repo_name}/{pattern}",
            real_time=0,
            error="All iterations failed"
        )

    return BenchmarkResult(
        name=f"search/{repo_name}/{pattern}",
        real_time=statistics.mean(times),
        iterations=len(times),
        metadata={
            "pattern": pattern,
            "repo": repo_name,
            "min": min(times),
            "max": max(times),
            "stddev": statistics.stdev(times) if len(times) > 1 else 0
        }
    )

def benchmark_get_overview(repo_name: str, repo_path: Path, iterations: int = 10) -> BenchmarkResult:
    """Benchmark get_overview operation"""
    times = []
    for _ in range(iterations):
        cmd = [str(ENGINE_BIN), "--get-overview", "--dir", str(repo_path)]
        duration, success, _ = run_with_timing(cmd, timeout=30)
        if success:
            times.append(duration)

    if not times:
        return BenchmarkResult(name=f"overview/{repo_name}", real_time=0, error="Failed")

    return BenchmarkResult(
        name=f"overview/{repo_name}",
        real_time=statistics.mean(times),
        iterations=len(times),
        metadata={"min": min(times), "max": max(times)}
    )

def benchmark_get_call_graph(repo_name: str, repo_path: Path, iterations: int = 10) -> BenchmarkResult:
    """Benchmark call graph retrieval"""
    times = []
    for _ in range(iterations):
        cmd = [str(ENGINE_BIN), "--get-call-graph", "--dir", str(repo_path)]
        duration, success, _ = run_with_timing(cmd, timeout=60)
        if success:
            times.append(duration)

    if not times:
        return BenchmarkResult(name=f"call_graph/{repo_name}", real_time=0, error="Failed")

    return BenchmarkResult(
        name=f"call_graph/{repo_name}",
        real_time=statistics.mean(times),
        iterations=len(times),
        metadata={"min": min(times), "max": max(times)}
    )

# ============================================================================
# Test Suites
# ============================================================================

def run_indexing_benchmarks(repos: list[tuple[str, Path]], parallel: bool = True) -> list[BenchmarkResult]:
    """Run indexing benchmarks on repos"""
    results = []

    print_progress(f"\n{'='*60}")
    print_progress("  INDEXING BENCHMARKS")
    print_progress(f"  Repos: {len(repos)}, Parallel: {parallel}")
    print_progress(f"{'='*60}\n")

    # Phase 1: Sequential baseline
    print_progress("Phase 1: Sequential indexing (baseline)...", "yellow")
    seq_start = time.perf_counter()

    for name, path in repos:
        result = benchmark_index_repo((name, path, True))
        results.append(result)
        status = "✓" if not result.error else "✗"
        print(f"  {status} {name}: {result.real_time:.2f}s ({result.items_per_second:.0f} files/s)")

    seq_total = time.perf_counter() - seq_start
    results.append(BenchmarkResult(
        name="indexing/sequential_total",
        real_time=seq_total,
        metadata={"repos": len(repos)}
    ))

    # Phase 2: Parallel indexing
    if parallel and len(repos) > 1:
        print_progress("\nPhase 2: Parallel indexing...", "yellow")

        # Clear all caches first
        for _, path in repos:
            clear_cache(path)

        par_start = time.perf_counter()

        with ProcessPoolExecutor(max_workers=min(len(repos), os.cpu_count() or 4)) as executor:
            futures = {
                executor.submit(benchmark_index_repo, (name, path, False)): name
                for name, path in repos
            }

            for future in as_completed(futures):
                name = futures[future]
                try:
                    result = future.result()
                    result.name = f"indexing_parallel/{result.metadata.get('repo', name)}"
                    results.append(result)
                    status = "✓" if not result.error else "✗"
                    print(f"  {status} {name}: {result.real_time:.2f}s")
                except Exception as e:
                    print(f"  ✗ {name}: {e}")

        par_total = time.perf_counter() - par_start
        speedup = seq_total / par_total if par_total > 0 else 1.0

        results.append(BenchmarkResult(
            name="indexing/parallel_total",
            real_time=par_total,
            metadata={"repos": len(repos), "speedup": round(speedup, 2)}
        ))

        print_progress(f"\n  Sequential: {seq_total:.2f}s", "green")
        print_progress(f"  Parallel:   {par_total:.2f}s", "green")
        print_progress(f"  Speedup:    {speedup:.2f}x", "green")

    return results

def run_query_benchmarks(repos: list[tuple[str, Path]], iterations: int = 5) -> list[BenchmarkResult]:
    """Run query benchmarks"""
    results = []

    print_progress(f"\n{'='*60}")
    print_progress("  QUERY BENCHMARKS")
    print_progress(f"  Repos: {len(repos)}, Iterations: {iterations}")
    print_progress(f"{'='*60}\n")

    # Ensure repos are indexed first
    print_progress("Ensuring indexes exist...", "yellow")
    for name, path in repos:
        cmd = [str(ENGINE_BIN), "--dir", str(path), "--format", "toon"]
        subprocess.run(cmd, capture_output=True)

    # Build list of all search benchmarks to run in parallel
    search_tasks = []
    for name, path in repos:
        for pattern in SEARCH_PATTERNS[:4]:  # Use subset for speed
            search_tasks.append((name, path, pattern, iterations))

    print_progress(f"Running {len(search_tasks)} search benchmarks in parallel...", "yellow")

    with ThreadPoolExecutor(max_workers=8) as executor:
        futures = [executor.submit(benchmark_search, task) for task in search_tasks]

        for future in as_completed(futures):
            try:
                result = future.result()
                results.append(result)
                if result.error:
                    print(f"  ✗ {result.name}: {result.error}")
                else:
                    print(f"  ✓ {result.name}: {result.real_time*1000:.1f}ms")
            except Exception as e:
                print(f"  ✗ Error: {e}")

    # Overview and call graph benchmarks
    print_progress("\nRunning overview/call_graph benchmarks...", "yellow")
    for name, path in repos[:3]:  # Limit to 3 repos
        result = benchmark_get_overview(name, path, iterations)
        results.append(result)
        print(f"  ✓ overview/{name}: {result.real_time*1000:.1f}ms")

        result = benchmark_get_call_graph(name, path, iterations)
        results.append(result)
        print(f"  ✓ call_graph/{name}: {result.real_time*1000:.1f}ms")

    return results

def run_stress_test(repos: list[tuple[str, Path]], num_queries: int = 100) -> list[BenchmarkResult]:
    """Run concurrent query stress test"""
    results = []

    print_progress(f"\n{'='*60}")
    print_progress("  STRESS TEST")
    print_progress(f"  Repos: {len(repos)}, Queries: {num_queries}")
    print_progress(f"{'='*60}\n")

    # Ensure indexes exist
    for _, path in repos:
        subprocess.run([str(ENGINE_BIN), "--dir", str(path), "--format", "toon"], capture_output=True)

    print_progress(f"Running {num_queries} concurrent queries...", "yellow")

    import random

    def random_query(_):
        name, path = random.choice(repos)
        pattern = random.choice(SEARCH_PATTERNS)
        cmd = [str(ENGINE_BIN), "--search-symbols", pattern, "--dir", str(path), "--limit", "20"]
        start = time.perf_counter()
        try:
            subprocess.run(cmd, capture_output=True, timeout=30)
            return time.perf_counter() - start, True
        except:
            return time.perf_counter() - start, False

    start_time = time.perf_counter()

    with ThreadPoolExecutor(max_workers=16) as executor:
        query_results = list(executor.map(random_query, range(num_queries)))

    total_time = time.perf_counter() - start_time

    successful = [t for t, ok in query_results if ok]
    failed = num_queries - len(successful)

    qps = num_queries / total_time if total_time > 0 else 0
    avg_latency = statistics.mean(successful) if successful else 0

    print_progress(f"\n  Total time:    {total_time:.2f}s", "green")
    print_progress(f"  Queries/sec:   {qps:.1f}", "green")
    print_progress(f"  Avg latency:   {avg_latency*1000:.1f}ms", "green")
    print_progress(f"  Success rate:  {len(successful)}/{num_queries}", "green")

    results.append(BenchmarkResult(
        name="stress/concurrent_queries",
        real_time=total_time,
        iterations=num_queries,
        items_per_second=qps,
        metadata={
            "avg_latency_ms": round(avg_latency * 1000, 2),
            "min_latency_ms": round(min(successful) * 1000, 2) if successful else 0,
            "max_latency_ms": round(max(successful) * 1000, 2) if successful else 0,
            "success_count": len(successful),
            "failure_count": failed
        }
    ))

    return results

# ============================================================================
# Report Generation
# ============================================================================

def generate_html_report(report: BenchmarkReport, output_path: Path):
    """Generate HTML report with charts"""

    # Group benchmarks by category
    categories = {}
    for b in report.benchmarks:
        cat = b.name.split("/")[0]
        if cat not in categories:
            categories[cat] = []
        categories[cat].append(b)

    html = f"""<!DOCTYPE html>
<html>
<head>
    <title>Semfora Performance Report - {report.context.date[:10]}</title>
    <script src="https://cdn.jsdelivr.net/npm/chart.js"></script>
    <style>
        body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; margin: 40px; background: #f5f5f5; }}
        .container {{ max-width: 1200px; margin: 0 auto; }}
        h1 {{ color: #333; }}
        .card {{ background: white; border-radius: 8px; padding: 20px; margin: 20px 0; box-shadow: 0 2px 4px rgba(0,0,0,0.1); }}
        .context {{ display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 10px; }}
        .context-item {{ background: #f8f9fa; padding: 10px; border-radius: 4px; }}
        .context-label {{ font-size: 12px; color: #666; }}
        .context-value {{ font-weight: bold; color: #333; }}
        table {{ width: 100%; border-collapse: collapse; }}
        th, td {{ padding: 12px; text-align: left; border-bottom: 1px solid #eee; }}
        th {{ background: #f8f9fa; font-weight: 600; }}
        .chart-container {{ height: 300px; margin: 20px 0; }}
        .success {{ color: #28a745; }}
        .error {{ color: #dc3545; }}
        .metric {{ font-size: 24px; font-weight: bold; color: #007bff; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>🚀 Semfora Performance Report</h1>

        <div class="card">
            <h2>System Context</h2>
            <div class="context">
                <div class="context-item">
                    <div class="context-label">Date</div>
                    <div class="context-value">{report.context.date[:19]}</div>
                </div>
                <div class="context-item">
                    <div class="context-label">Host</div>
                    <div class="context-value">{report.context.host_name}</div>
                </div>
                <div class="context-item">
                    <div class="context-label">CPUs</div>
                    <div class="context-value">{report.context.num_cpus}</div>
                </div>
                <div class="context-item">
                    <div class="context-label">Memory</div>
                    <div class="context-value">{report.context.memory_gb} GB</div>
                </div>
                <div class="context-item">
                    <div class="context-label">Git Commit</div>
                    <div class="context-value">{report.context.git_commit or 'N/A'}</div>
                </div>
                <div class="context-item">
                    <div class="context-label">Rust</div>
                    <div class="context-value">{report.context.rust_version.split()[1] if report.context.rust_version else 'N/A'}</div>
                </div>
            </div>
        </div>
"""

    # Add sections for each category
    for cat, benchmarks in categories.items():
        html += f"""
        <div class="card">
            <h2>{cat.replace('_', ' ').title()} Benchmarks</h2>
            <table>
                <tr>
                    <th>Name</th>
                    <th>Time</th>
                    <th>Iterations</th>
                    <th>Throughput</th>
                    <th>Status</th>
                </tr>
"""
        for b in benchmarks:
            time_str = f"{b.real_time:.3f}s" if b.real_time >= 1 else f"{b.real_time*1000:.1f}ms"
            throughput = f"{b.items_per_second:.1f}/s" if b.items_per_second > 0 else "-"
            status = '<span class="error">✗</span>' if b.error else '<span class="success">✓</span>'

            html += f"""
                <tr>
                    <td>{b.name}</td>
                    <td>{time_str}</td>
                    <td>{b.iterations}</td>
                    <td>{throughput}</td>
                    <td>{status}</td>
                </tr>
"""
        html += """
            </table>
        </div>
"""

    html += """
    </div>
</body>
</html>
"""

    output_path.write_text(html)
    print_progress(f"\nHTML report: {output_path}", "green")

# ============================================================================
# Main
# ============================================================================

def build_release():
    """Build release binaries"""
    print_progress("Building release binaries...", "yellow")
    result = subprocess.run(
        ["cargo", "build", "--release"],
        cwd=PROJECT_DIR,
        capture_output=True
    )
    if result.returncode != 0:
        print_progress("Build failed!", "red")
        print(result.stderr.decode())
        sys.exit(1)
    print_progress("Build complete.", "green")

def main():
    parser = argparse.ArgumentParser(description="Semfora Performance Test Suite")
    parser.add_argument("--quick", action="store_true", help="Quick smoke test (small repos only)")
    parser.add_argument("--indexing-only", action="store_true", help="Run only indexing benchmarks")
    parser.add_argument("--queries-only", action="store_true", help="Run only query benchmarks")
    parser.add_argument("--stress-only", action="store_true", help="Run only stress test")
    parser.add_argument("--no-build", action="store_true", help="Skip cargo build")
    parser.add_argument("--report", action="store_true", help="Generate HTML report from latest results")
    parser.add_argument("--output", type=str, help="Output JSON file path")
    args = parser.parse_args()

    # Setup
    RESULTS_DIR.mkdir(parents=True, exist_ok=True)
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")

    if args.report:
        # Find latest results and generate report
        json_files = sorted(RESULTS_DIR.glob("perf_*.json"), reverse=True)
        if not json_files:
            print_progress("No results found to generate report from", "red")
            sys.exit(1)

        with open(json_files[0]) as f:
            data = json.load(f)

        context = BenchmarkContext(**data["context"])
        benchmarks = [BenchmarkResult(**b) for b in data["benchmarks"]]
        report = BenchmarkReport(context=context, benchmarks=benchmarks)

        html_path = RESULTS_DIR / f"report_{timestamp}.html"
        generate_html_report(report, html_path)
        return

    # Build
    if not args.no_build:
        build_release()

    if not ENGINE_BIN.exists():
        print_progress(f"Engine binary not found: {ENGINE_BIN}", "red")
        sys.exit(1)

    # Find repos
    if args.quick:
        repos = find_repos("small")[:3]
    else:
        repos = find_repos("all")

    if not repos:
        print_progress(f"No test repos found in {REPOS_DIR}", "red")
        print_progress("Set SEMFORA_TEST_REPOS environment variable", "yellow")
        sys.exit(1)

    print_progress(f"\n{'='*60}")
    print_progress("  SEMFORA PERFORMANCE TEST SUITE")
    print_progress(f"  Timestamp: {timestamp}")
    print_progress(f"  Repos: {len(repos)}")
    print_progress(f"{'='*60}")

    # Gather context
    context = get_system_context()
    all_results = []

    # Run benchmarks
    if args.indexing_only:
        all_results.extend(run_indexing_benchmarks(repos))
    elif args.queries_only:
        all_results.extend(run_query_benchmarks(repos[:5]))
    elif args.stress_only:
        all_results.extend(run_stress_test(repos[:5]))
    else:
        # Run all
        all_results.extend(run_indexing_benchmarks(repos))
        all_results.extend(run_query_benchmarks(repos[:5]))
        all_results.extend(run_stress_test(repos[:5]))

    # Create report
    report = BenchmarkReport(context=context, benchmarks=all_results)

    # Save JSON
    output_path = Path(args.output) if args.output else RESULTS_DIR / f"perf_{timestamp}.json"
    with open(output_path, "w") as f:
        json.dump(report.to_dict(), f, indent=2)

    print_progress(f"\n{'='*60}")
    print_progress("  COMPLETE")
    print_progress(f"  JSON: {output_path}")
    print_progress(f"{'='*60}")

    # Generate HTML report
    html_path = RESULTS_DIR / f"report_{timestamp}.html"
    generate_html_report(report, html_path)

if __name__ == "__main__":
    main()
