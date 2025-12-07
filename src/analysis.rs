//! Static code analysis module
//!
//! Provides complexity metrics, call graph analysis, and code health reports
//! built on top of the semantic index.

use crate::cache::CacheDir;
use crate::schema::{RiskLevel, SemanticSummary, SymbolKind};
use crate::Result;
use std::collections::HashMap;
use std::path::Path;

/// Complexity metrics for a single symbol
#[derive(Debug, Clone, Default)]
pub struct SymbolComplexity {
    /// Symbol name
    pub name: String,
    /// Symbol hash for stable identification
    pub hash: String,
    /// File containing the symbol
    pub file: String,
    /// Line range (start-end)
    pub lines: String,
    /// Symbol kind
    pub kind: SymbolKind,

    // Complexity metrics
    /// Cyclomatic complexity (control flow paths)
    pub cyclomatic: usize,
    /// Cognitive complexity (SonarSource metric - accounts for nesting)
    pub cognitive: usize,
    /// Number of function calls made (fan-out)
    pub fan_out: usize,
    /// Number of callers (fan-in) - populated during graph analysis
    pub fan_in: usize,
    /// Number of state mutations
    pub state_mutations: usize,
    /// Lines of code
    pub loc: usize,
    /// Maximum nesting depth
    pub max_nesting: usize,
    /// Behavioral risk level (deprecated - use cognitive complexity instead)
    pub risk: RiskLevel,

    // Dependency metrics
    /// Number of imports/dependencies
    pub dependencies: usize,
    /// I/O operations detected
    pub io_operations: usize,
}

/// Calculate cognitive complexity from control flow changes
///
/// Cognitive complexity (SonarSource) is calculated as:
/// - +1 for each control flow break (if, for, while, switch, try, etc.)
/// - +1 additional for each level of nesting
///
/// Example:
/// ```ignore
/// if (a) {           // +1 (depth 0)
///     if (b) {       // +1 + 1 = +2 (depth 1)
///         for (c) {  // +1 + 2 = +3 (depth 2)
///         }
///     }
/// }
/// // Total: 1 + 2 + 3 = 6
/// ```
pub fn calculate_cognitive_complexity(control_flow: &[crate::schema::ControlFlowChange]) -> usize {
    let mut complexity = 0;

    for cf in control_flow {
        // Base increment for the control structure
        let base = 1;
        // Nesting penalty
        let nesting_penalty = cf.nesting_depth;
        complexity += base + nesting_penalty;
    }

    complexity
}

/// Get the maximum nesting depth from control flow changes
pub fn max_nesting_depth(control_flow: &[crate::schema::ControlFlowChange]) -> usize {
    control_flow.iter().map(|cf| cf.nesting_depth).max().unwrap_or(0)
}

impl SymbolComplexity {
    /// Calculate a composite complexity score using cognitive complexity as primary metric
    pub fn complexity_score(&self) -> usize {
        // Cognitive complexity is the primary metric (already accounts for nesting)
        let mut score = self.cognitive;

        // High fan-out suggests tight coupling
        if self.fan_out > 10 {
            score += (self.fan_out - 10) / 2;
        }

        // Long functions are harder to maintain
        if self.loc > 50 {
            score += (self.loc - 50) / 25;
        }

        score
    }

    /// Get a human-readable complexity rating based on cognitive complexity
    ///
    /// Thresholds based on SonarSource recommendations:
    /// - 0-5: simple (easy to understand)
    /// - 6-10: moderate (some effort to understand)
    /// - 11-20: complex (hard to understand, consider refactoring)
    /// - 21+: very complex (should be refactored)
    pub fn rating(&self) -> &'static str {
        match self.cognitive {
            0..=5 => "simple",
            6..=10 => "moderate",
            11..=20 => "complex",
            _ => "very complex",
        }
    }

    /// Get a color hint for the rating (for terminal output)
    pub fn rating_color(&self) -> &'static str {
        match self.cognitive {
            0..=5 => "green",
            6..=10 => "yellow",
            11..=20 => "orange",
            _ => "red",
        }
    }
}

/// Module-level metrics aggregated from symbols
#[derive(Debug, Clone, Default)]
pub struct ModuleMetrics {
    /// Module name
    pub name: String,
    /// Total files in module
    pub files: usize,
    /// Total symbols in module
    pub symbols: usize,
    /// Average cyclomatic complexity
    pub avg_complexity: f64,
    /// Max cyclomatic complexity
    pub max_complexity: usize,
    /// Symbol with highest complexity
    pub most_complex_symbol: Option<String>,
    /// Total lines of code
    pub total_loc: usize,
    /// Number of high-risk symbols
    pub high_risk_count: usize,
    /// Afferent coupling (incoming dependencies from other modules)
    pub afferent_coupling: usize,
    /// Efferent coupling (outgoing dependencies to other modules)
    pub efferent_coupling: usize,
}

impl ModuleMetrics {
    /// Calculate instability metric (Ce / (Ca + Ce))
    /// 0 = maximally stable (hard to change)
    /// 1 = maximally unstable (easy to change)
    pub fn instability(&self) -> f64 {
        let total = self.afferent_coupling + self.efferent_coupling;
        if total == 0 {
            0.5 // Neutral if no coupling
        } else {
            self.efferent_coupling as f64 / total as f64
        }
    }
}

/// Call graph analysis results
#[derive(Debug, Clone, Default)]
pub struct CallGraphAnalysis {
    /// Symbols with highest fan-in (most called)
    pub hotspots: Vec<(String, usize)>,
    /// Symbols with highest fan-out (most dependencies)
    pub high_coupling: Vec<(String, usize)>,
    /// Circular dependency chains detected
    pub cycles: Vec<Vec<String>>,
    /// Orphan symbols (no callers, no callees)
    pub orphans: Vec<String>,
    /// Entry points (high fan-in, low fan-out)
    pub entry_points: Vec<String>,
    /// Leaf functions (called but don't call others)
    pub leaf_functions: Vec<String>,
}

/// Repository-wide analysis summary
#[derive(Debug, Clone, Default)]
pub struct RepoAnalysis {
    /// Per-module metrics
    pub modules: Vec<ModuleMetrics>,
    /// Top 20 most complex symbols
    pub complex_symbols: Vec<SymbolComplexity>,
    /// Call graph analysis
    pub call_graph: CallGraphAnalysis,
    /// Overall stats
    pub total_symbols: usize,
    pub total_lines: usize,
    pub avg_complexity: f64,
    pub high_risk_percentage: f64,
}

/// Analyze complexity from a call graph
pub fn analyze_call_graph(
    call_graph: &HashMap<String, Vec<String>>,
    symbol_names: &HashMap<String, String>, // hash -> name
) -> CallGraphAnalysis {
    let mut analysis = CallGraphAnalysis::default();

    // Build reverse graph for fan-in calculation
    let mut fan_in: HashMap<String, usize> = HashMap::new();
    let mut fan_out: HashMap<String, usize> = HashMap::new();

    for (caller, callees) in call_graph {
        fan_out.insert(caller.clone(), callees.len());
        for callee in callees {
            *fan_in.entry(callee.clone()).or_insert(0) += 1;
        }
    }

    // Find hotspots (high fan-in)
    let mut hotspots: Vec<_> = fan_in.iter()
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    hotspots.sort_by(|a, b| b.1.cmp(&a.1));
    analysis.hotspots = hotspots.into_iter().take(10).collect();

    // Find high coupling (high fan-out) - resolve hash to name
    let mut high_coupling: Vec<_> = fan_out.iter()
        .filter(|(_, v)| **v > 10)
        .map(|(hash, v)| {
            let name = symbol_names.get(hash).cloned().unwrap_or_else(|| hash.clone());
            (name, *v)
        })
        .collect();
    high_coupling.sort_by(|a, b| b.1.cmp(&a.1));
    analysis.high_coupling = high_coupling;

    // Find orphans (no fan-in, no fan-out except self)
    for (hash, _) in symbol_names {
        let fi = fan_in.get(hash).copied().unwrap_or(0);
        let fo = fan_out.get(hash).copied().unwrap_or(0);
        if fi == 0 && fo == 0 {
            if let Some(name) = symbol_names.get(hash) {
                analysis.orphans.push(name.clone());
            }
        }
    }

    // Find entry points (high fan-in, low fan-out)
    for (hash, &fi) in &fan_in {
        let fo = fan_out.get(hash).copied().unwrap_or(0);
        if fi >= 5 && fo <= 2 {
            if let Some(name) = symbol_names.get(hash) {
                analysis.entry_points.push(name.clone());
            }
        }
    }

    // Find leaf functions (called, but don't call others)
    for (hash, &fi) in &fan_in {
        let fo = fan_out.get(hash).copied().unwrap_or(0);
        if fi > 0 && fo == 0 {
            if let Some(name) = symbol_names.get(hash) {
                analysis.leaf_functions.push(name.clone());
            }
        }
    }

    // Cycle detection using DFS
    analysis.cycles = detect_cycles(call_graph);

    analysis
}

/// Detect cycles in call graph using DFS
fn detect_cycles(graph: &HashMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut cycles = Vec::new();
    let mut visited = HashMap::new();
    let mut rec_stack = HashMap::new();
    let mut path = Vec::new();

    fn dfs(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashMap<String, bool>,
        rec_stack: &mut HashMap<String, bool>,
        path: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
    ) {
        visited.insert(node.to_string(), true);
        rec_stack.insert(node.to_string(), true);
        path.push(node.to_string());

        if let Some(neighbors) = graph.get(node) {
            for neighbor in neighbors {
                if !visited.get(neighbor).copied().unwrap_or(false) {
                    dfs(neighbor, graph, visited, rec_stack, path, cycles);
                } else if rec_stack.get(neighbor).copied().unwrap_or(false) {
                    // Found a cycle - extract it
                    if let Some(start_idx) = path.iter().position(|x| x == neighbor) {
                        let cycle: Vec<String> = path[start_idx..].to_vec();
                        if cycle.len() > 1 && cycle.len() <= 5 {
                            // Only report small cycles (2-5 nodes)
                            cycles.push(cycle);
                        }
                    }
                }
            }
        }

        path.pop();
        rec_stack.insert(node.to_string(), false);
    }

    for node in graph.keys() {
        if !visited.get(node).copied().unwrap_or(false) {
            dfs(node, graph, &mut visited, &mut rec_stack, &mut path, &mut cycles);
        }
    }

    cycles
}

/// Format analysis as text report
pub fn format_analysis_report(analysis: &RepoAnalysis) -> String {
    let mut output = String::new();

    output.push_str("╔══════════════════════════════════════════════════════════════════╗\n");
    output.push_str("║                    STATIC CODE ANALYSIS REPORT                   ║\n");
    output.push_str("╚══════════════════════════════════════════════════════════════════╝\n\n");

    // Overview
    output.push_str("── OVERVIEW ─────────────────────────────────────────────────────────\n");
    output.push_str(&format!("  Total Symbols:     {:>6}\n", analysis.total_symbols));
    output.push_str(&format!("  Total Lines:       {:>6}\n", analysis.total_lines));
    output.push_str(&format!("  Avg Cognitive:     {:>6.1}\n", analysis.avg_complexity));
    output.push_str("\n");

    // Top Complex Symbols - sorted by cognitive complexity
    output.push_str("── COGNITIVE COMPLEXITY HOTSPOTS ────────────────────────────────────\n");
    output.push_str("  Symbol                           Cog  Nest   LoC  FanOut  Rating\n");
    output.push_str("  ─────────────────────────────────────────────────────────────────\n");

    for sym in analysis.complex_symbols.iter().take(15) {
        let name = if sym.name.len() > 30 {
            format!("{}...", &sym.name[..27])
        } else {
            sym.name.clone()
        };
        output.push_str(&format!(
            "  {:<30} {:>4}  {:>4}  {:>4}  {:>6}   {}\n",
            name, sym.cognitive, sym.max_nesting, sym.loc, sym.fan_out, sym.rating()
        ));
    }
    output.push_str("\n");

    // Legend
    output.push_str("  Cog: Cognitive Complexity (0-5 simple, 6-10 moderate, 11-20 complex, 21+ very complex)\n");
    output.push_str("  Nest: Maximum nesting depth | LoC: Lines of code | FanOut: Function calls\n");
    output.push_str("\n");

    // Module Analysis
    output.push_str("── MODULE METRICS ───────────────────────────────────────────────────\n");
    output.push_str("  Module              Symbols  AvgCC  MaxCC  LoC    Instability\n");
    output.push_str("  ─────────────────────────────────────────────────────────────────\n");

    let mut modules: Vec<_> = analysis.modules.iter().collect();
    modules.sort_by(|a, b| b.avg_complexity.partial_cmp(&a.avg_complexity).unwrap());

    for m in modules.iter().take(15) {
        let name = if m.name.len() > 18 {
            format!("{}...", &m.name[..15])
        } else {
            m.name.clone()
        };
        output.push_str(&format!(
            "  {:<18} {:>7}  {:>5.1}  {:>5}  {:>5}  {:>10.2}\n",
            name, m.symbols, m.avg_complexity, m.max_complexity,
            m.total_loc, m.instability()
        ));
    }
    output.push_str("\n");

    // Call Graph Analysis
    output.push_str("── CALL GRAPH ANALYSIS ──────────────────────────────────────────────\n");

    if !analysis.call_graph.hotspots.is_empty() {
        output.push_str("  Hotspots (most called):\n");
        for (name, count) in analysis.call_graph.hotspots.iter().take(10) {
            let display = if name.len() > 40 { &name[..40] } else { name };
            output.push_str(&format!("    {:<40} ({} callers)\n", display, count));
        }
        output.push_str("\n");
    }

    if !analysis.call_graph.high_coupling.is_empty() {
        output.push_str("  High Coupling (many outgoing calls):\n");
        for (name, count) in analysis.call_graph.high_coupling.iter().take(5) {
            let display = if name.len() > 40 { &name[..40] } else { name };
            output.push_str(&format!("    {:<40} ({} callees)\n", display, count));
        }
        output.push_str("\n");
    }

    if !analysis.call_graph.cycles.is_empty() {
        output.push_str("  ⚠ Circular Dependencies Detected:\n");
        for cycle in analysis.call_graph.cycles.iter().take(5) {
            output.push_str(&format!("    {} → {}\n",
                cycle.join(" → "),
                cycle.first().unwrap_or(&String::new())
            ));
        }
        output.push_str("\n");
    }

    if !analysis.call_graph.entry_points.is_empty() {
        output.push_str(&format!("  Entry Points: {} symbols\n",
            analysis.call_graph.entry_points.len()));
    }

    if !analysis.call_graph.leaf_functions.is_empty() {
        output.push_str(&format!("  Leaf Functions: {} symbols\n",
            analysis.call_graph.leaf_functions.len()));
    }

    output.push_str("\n");
    output.push_str("══════════════════════════════════════════════════════════════════════\n");

    output
}

/// Build SymbolComplexity from a SemanticSummary
fn symbol_complexity_from_summary(summary: &SemanticSummary, fan_in: usize) -> SymbolComplexity {
    let loc = match (summary.start_line, summary.end_line) {
        (Some(s), Some(e)) => e.saturating_sub(s) + 1,
        _ => 0,
    };

    // Cyclomatic complexity: base 1 + one per control flow branch
    let cyclomatic = 1 + summary.control_flow_changes.len();

    // Cognitive complexity: accounts for nesting depth
    let cognitive = calculate_cognitive_complexity(&summary.control_flow_changes);

    // Maximum nesting depth from actual control flow data
    let max_nesting = max_nesting_depth(&summary.control_flow_changes);

    // Count I/O operations
    let io_operations = summary.calls.iter()
        .filter(|c| crate::schema::Call::check_is_io(&c.name))
        .count();

    SymbolComplexity {
        name: summary.symbol.clone().unwrap_or_default(),
        hash: summary.symbol_id.as_ref().map(|id| id.hash.clone()).unwrap_or_default(),
        file: summary.file.clone(),
        lines: format!("{}-{}",
            summary.start_line.unwrap_or(0),
            summary.end_line.unwrap_or(0)),
        kind: summary.symbol_kind.clone().unwrap_or_default(),
        cyclomatic,
        cognitive,
        fan_out: summary.calls.len(),
        fan_in,
        state_mutations: summary.state_changes.len(),
        loc,
        max_nesting,
        risk: summary.behavioral_risk,
        dependencies: summary.added_dependencies.len(),
        io_operations,
    }
}

/// Analyze a repository from its cached index
///
/// This is the main entry point for static analysis. It reads from the
/// pre-built semantic index and computes complexity metrics.
/// Parse a lines string "start-end" into (start, end)
fn parse_lines(lines: &str) -> (usize, usize) {
    let parts: Vec<&str> = lines.split('-').collect();
    if parts.len() == 2 {
        let start = parts[0].parse().unwrap_or(0);
        let end = parts[1].parse().unwrap_or(0);
        (start, end)
    } else {
        (0, 0)
    }
}

pub fn analyze_repo(repo_path: &Path) -> Result<RepoAnalysis> {
    let cache = CacheDir::for_repo(repo_path)?;
    let mut analysis = RepoAnalysis::default();

    // Load call graph for fan-in/fan-out calculation
    let call_graph = cache.load_call_graph().unwrap_or_default();

    // Build reverse map for fan-in
    let mut fan_in_map: HashMap<String, usize> = HashMap::new();
    let mut fan_out_map: HashMap<String, usize> = HashMap::new();
    for (caller, callees) in &call_graph {
        fan_out_map.insert(caller.clone(), callees.len());
        for callee in callees {
            *fan_in_map.entry(callee.clone()).or_insert(0) += 1;
        }
    }

    // Build symbol name map for call graph analysis
    let mut symbol_names: HashMap<String, String> = HashMap::new();

    // Load all symbol entries from the index
    let symbol_entries = cache.load_all_symbol_entries().unwrap_or_default();

    // Build map of hash -> (file, start, end) for aggregating fan_out in impl blocks
    let mut hash_to_location: HashMap<String, (&str, usize, usize)> = HashMap::new();
    for entry in &symbol_entries {
        let (start, end) = parse_lines(&entry.lines);
        hash_to_location.insert(entry.hash.clone(), (&entry.file, start, end));
    }

    // Track name -> max fan_out for direct name-based lookup
    // This handles impl blocks where calls are attributed to the struct with same name
    let mut name_to_fan_out: HashMap<String, usize> = HashMap::new();
    for entry in &symbol_entries {
        // Track max fan_out for each symbol name (struct might have higher than impl)
        if let Some(&fo) = fan_out_map.get(&entry.hash) {
            let current = name_to_fan_out.entry(entry.symbol.clone()).or_insert(0);
            *current = (*current).max(fo);
        }
    }

    // Group entries by module
    let mut module_entries: HashMap<String, Vec<&crate::cache::SymbolIndexEntry>> = HashMap::new();
    for entry in &symbol_entries {
        module_entries.entry(entry.module.clone())
            .or_default()
            .push(entry);
        symbol_names.insert(entry.hash.clone(), entry.symbol.clone());
    }

    // Build map of file -> total fan_out for same-file aggregation fallback
    let mut file_to_fan_out: HashMap<String, usize> = HashMap::new();
    for (cg_hash, &fo) in &fan_out_map {
        if let Some(&(cg_file, _, _)) = hash_to_location.get(cg_hash) {
            *file_to_fan_out.entry(cg_file.to_string()).or_insert(0) += fo;
        }
    }

    // Helper closure to get fan_out for a symbol
    // 1. Try direct hash lookup
    // 2. Try looking up by symbol name (for impl blocks where calls are attributed to struct)
    // 3. Aggregate from functions within line range
    // 4. For large impl blocks, use file-level fan_out if symbol dominates the file
    let get_fan_out = |hash: &str, name: &str, kind: &str, file: &str, start: usize, end: usize| -> usize {
        // First try direct lookup
        if let Some(&fo) = fan_out_map.get(hash) {
            return fo;
        }
        // Try by symbol name (impl blocks inherit fan_out from struct)
        if let Some(&fo) = name_to_fan_out.get(name) {
            return fo;
        }
        // Aggregate from functions within this symbol's line range
        let mut total = 0;
        for (cg_hash, &fo) in &fan_out_map {
            if let Some(&(cg_file, cg_start, cg_end)) = hash_to_location.get(cg_hash) {
                if cg_file == file && cg_start >= start && cg_end <= end {
                    total += fo;
                }
            }
        }
        if total > 0 {
            return total;
        }
        // For impl blocks (kind=method) spanning many lines, aggregate from same file
        // This handles cases where calls are attributed to a different symbol in same file
        if kind == "method" && (end - start) > 100 {
            if let Some(&fo) = file_to_fan_out.get(file) {
                return fo;
            }
        }
        0
    };

    // Process modules
    for (module_name, entries) in &module_entries {
        let mut module_metrics = ModuleMetrics {
            name: module_name.clone(),
            ..Default::default()
        };

        let mut files_seen = std::collections::HashSet::new();
        let mut module_cc_sum = 0usize;

        for entry in entries {
            files_seen.insert(&entry.file);

            // Parse line range for LoC
            let (start, end) = parse_lines(&entry.lines);
            let loc = if end > start { end - start + 1 } else { 1 };

            // Get fan-in/fan-out (looks up by name for impl blocks where calls are on struct)
            let fan_in = fan_in_map.get(&entry.hash).copied().unwrap_or(0);
            let fan_out = get_fan_out(&entry.hash, &entry.symbol, &entry.kind, &entry.file, start, end);

            let sym_complexity = SymbolComplexity {
                name: entry.symbol.clone(),
                hash: entry.hash.clone(),
                file: entry.file.clone(),
                lines: entry.lines.clone(),
                kind: SymbolKind::from_str(&entry.kind),
                cyclomatic: entry.cognitive_complexity, // Using cognitive as proxy for cyclomatic
                cognitive: entry.cognitive_complexity,
                fan_out,
                fan_in,
                state_mutations: 0,
                loc,
                max_nesting: entry.max_nesting,
                risk: RiskLevel::from_str(&entry.risk),
                dependencies: 0,
                io_operations: 0,
            };

            module_metrics.total_loc += loc;
            module_cc_sum += entry.cognitive_complexity;

            // Track max complexity for module
            if entry.cognitive_complexity > module_metrics.max_complexity {
                module_metrics.max_complexity = entry.cognitive_complexity;
                module_metrics.most_complex_symbol = Some(entry.symbol.clone());
            }

            if entry.risk == "high" {
                module_metrics.high_risk_count += 1;
            }

            // Track symbols that are complex (cognitive > 5, high fan-out, or large)
            if entry.cognitive_complexity > 5 || fan_out > 10 || loc > 50 {
                analysis.complex_symbols.push(sym_complexity);
            }

            module_metrics.symbols += 1;
        }

        module_metrics.files = files_seen.len();
        if module_metrics.symbols > 0 {
            module_metrics.avg_complexity = module_cc_sum as f64 / module_metrics.symbols as f64;
        }
        analysis.total_symbols += module_metrics.symbols;
        analysis.total_lines += module_metrics.total_loc;
        analysis.modules.push(module_metrics);
    }

    // Calculate overall averages
    if analysis.total_symbols > 0 {
        // Average cognitive complexity across all symbols
        let total_cc: usize = symbol_entries.iter()
            .map(|e| e.cognitive_complexity)
            .sum();
        analysis.avg_complexity = total_cc as f64 / analysis.total_symbols as f64;

        let high_risk_count = symbol_entries.iter()
            .filter(|e| e.risk == "high")
            .count();
        analysis.high_risk_percentage = (high_risk_count as f64 / analysis.total_symbols as f64) * 100.0;
    }

    // Sort complex symbols by cognitive complexity (primary) then fan-out (secondary)
    analysis.complex_symbols.sort_by(|a, b| {
        let score_a = a.cognitive * 100 + a.fan_out * 10 + a.loc;
        let score_b = b.cognitive * 100 + b.fan_out * 10 + b.loc;
        score_b.cmp(&score_a)
    });
    analysis.complex_symbols.truncate(20);

    // Analyze call graph
    analysis.call_graph = analyze_call_graph(&call_graph, &symbol_names);

    Ok(analysis)
}

/// Quick complexity check for a single module
pub fn analyze_module(repo_path: &Path, module_name: &str) -> Result<ModuleMetrics> {
    let cache = CacheDir::for_repo(repo_path)?;
    let call_graph = cache.load_call_graph().unwrap_or_default();

    // Build fan-in map
    let mut fan_in_map: HashMap<String, usize> = HashMap::new();
    for (_caller, callees) in &call_graph {
        for callee in callees {
            *fan_in_map.entry(callee.clone()).or_insert(0) += 1;
        }
    }

    let summaries = cache.load_module_summaries(module_name)?;
    let mut metrics = ModuleMetrics {
        name: module_name.to_string(),
        ..Default::default()
    };

    let mut complexity_sum = 0usize;
    let mut files_seen = std::collections::HashSet::new();

    for summary in &summaries {
        files_seen.insert(&summary.file);

        let fan_in = summary.symbol_id.as_ref()
            .and_then(|id| fan_in_map.get(&id.hash).copied())
            .unwrap_or(0);

        let sym_complexity = symbol_complexity_from_summary(summary, fan_in);

        complexity_sum += sym_complexity.cyclomatic;
        metrics.total_loc += sym_complexity.loc;

        if sym_complexity.cyclomatic > metrics.max_complexity {
            metrics.max_complexity = sym_complexity.cyclomatic;
            metrics.most_complex_symbol = Some(sym_complexity.name.clone());
        }

        if matches!(summary.behavioral_risk, RiskLevel::High) {
            metrics.high_risk_count += 1;
        }

        metrics.symbols += 1;
    }

    metrics.files = files_seen.len();
    if metrics.symbols > 0 {
        metrics.avg_complexity = complexity_sum as f64 / metrics.symbols as f64;
    }

    Ok(metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_complexity_score() {
        let mut sym = SymbolComplexity::default();
        sym.cognitive = 1;
        assert_eq!(sym.rating(), "simple");

        sym.cognitive = 5;
        assert_eq!(sym.rating(), "simple");

        sym.cognitive = 6;
        assert_eq!(sym.rating(), "moderate");

        sym.cognitive = 10;
        assert_eq!(sym.rating(), "moderate");

        sym.cognitive = 11;
        assert_eq!(sym.rating(), "complex");

        sym.cognitive = 21;
        assert_eq!(sym.rating(), "very complex");
    }

    #[test]
    fn test_cycle_detection() {
        let mut graph = HashMap::new();
        graph.insert("a".to_string(), vec!["b".to_string()]);
        graph.insert("b".to_string(), vec!["c".to_string()]);
        graph.insert("c".to_string(), vec!["a".to_string()]);

        let cycles = detect_cycles(&graph);
        assert!(!cycles.is_empty());
    }

    #[test]
    fn test_instability() {
        let mut metrics = ModuleMetrics::default();
        metrics.afferent_coupling = 10;
        metrics.efferent_coupling = 10;
        assert!((metrics.instability() - 0.5).abs() < 0.01);

        metrics.afferent_coupling = 0;
        metrics.efferent_coupling = 10;
        assert!((metrics.instability() - 1.0).abs() < 0.01);
    }
}
