//! Generic Semantic Extractor
//!
//! This module provides language-agnostic semantic extraction using
//! the grammar definitions from `grammar.rs`. Instead of duplicating
//! extraction logic in each language detector, we use one implementation
//! that works with any tree-sitter grammar.
//!
//! # Usage
//!
//! ```ignore
//! use crate::detectors::generic::extract_with_grammar;
//! use crate::detectors::grammar::GO_GRAMMAR;
//!
//! extract_with_grammar(summary, source, tree, &GO_GRAMMAR)?;
//! ```

use tree_sitter::{Node, Tree};

use crate::detectors::common::{get_node_text, get_node_text_normalized};
use crate::detectors::grammar::LangGrammar;
use crate::error::Result;
use crate::schema::{
    Call, ControlFlowChange, ControlFlowKind, Location, RiskLevel, SemanticSummary, StateChange,
    SymbolInfo, SymbolKind,
};

// =============================================================================
// Main Entry Point
// =============================================================================

/// Extract semantic information from source code using the provided grammar
pub fn extract_with_grammar(
    summary: &mut SemanticSummary,
    source: &str,
    tree: &Tree,
    grammar: &LangGrammar,
) -> Result<()> {
    let root = tree.root_node();

    // Extract all semantic information
    extract_symbols(summary, &root, source, grammar);
    extract_imports(summary, &root, source, grammar);
    extract_state_changes(summary, &root, source, grammar);
    extract_control_flow(summary, &root, source, grammar);
    extract_calls(summary, &root, source, grammar);

    // Calculate derived metrics
    calculate_complexity(summary);
    determine_risk(summary);

    Ok(())
}

// =============================================================================
// Symbol Extraction
// =============================================================================

/// Candidate symbol for ranking
struct SymbolCandidate {
    name: String,
    kind: SymbolKind,
    is_exported: bool,
    start_line: usize,
    end_line: usize,
    score: i32,
}

/// Extract all symbols (functions, classes, interfaces, enums)
fn extract_symbols(summary: &mut SemanticSummary, root: &Node, source: &str, grammar: &LangGrammar) {
    let mut candidates: Vec<SymbolCandidate> = Vec::new();
    let filename_stem = extract_filename_stem(&summary.file);

    collect_symbols_recursive(root, source, grammar, &filename_stem, &mut candidates);

    // Sort by score (highest first)
    candidates.sort_by(|a, b| b.score.cmp(&a.score));

    // Convert to SymbolInfo and add to summary
    for candidate in &candidates {
        let symbol_info = SymbolInfo {
            name: candidate.name.clone(),
            kind: candidate.kind,
            start_line: candidate.start_line,
            end_line: candidate.end_line,
            is_exported: candidate.is_exported,
            is_default_export: false,
            hash: None,
            arguments: Vec::new(),
            props: Vec::new(),
            return_type: None,
            calls: Vec::new(),
            control_flow: Vec::new(),
            state_changes: Vec::new(),
            behavioral_risk: RiskLevel::Low,
        };
        summary.symbols.push(symbol_info);
    }

    // Set primary symbol (backward compatibility)
    if let Some(best) = candidates.first() {
        summary.symbol = Some(best.name.clone());
        summary.symbol_kind = Some(best.kind);
        summary.start_line = Some(best.start_line);
        summary.end_line = Some(best.end_line);
        summary.public_surface_changed = best.is_exported;
    }
}

fn collect_symbols_recursive(
    node: &Node,
    source: &str,
    grammar: &LangGrammar,
    filename_stem: &str,
    candidates: &mut Vec<SymbolCandidate>,
) {
    let kind_str = node.kind();

    // Check if this node is a symbol
    let symbol_kind = if grammar.function_nodes.contains(&kind_str) {
        Some(SymbolKind::Function)
    } else if grammar.class_nodes.contains(&kind_str) {
        Some(SymbolKind::Class)
    } else if grammar.interface_nodes.contains(&kind_str) {
        Some(SymbolKind::Trait)
    } else if grammar.enum_nodes.contains(&kind_str) {
        Some(SymbolKind::Enum)
    } else {
        None
    };

    if let Some(kind) = symbol_kind {
        if let Some(name) = extract_symbol_name(node, source, grammar) {
            let is_exported = (grammar.is_exported)(node, source);
            let score = calculate_symbol_score(&name, &kind, is_exported, filename_stem, grammar);

            candidates.push(SymbolCandidate {
                name,
                kind,
                is_exported,
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                score,
            });
        }
    }

    // Recurse into children
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_symbols_recursive(&child, source, grammar, filename_stem, candidates);
    }
}

fn extract_symbol_name(node: &Node, source: &str, grammar: &LangGrammar) -> Option<String> {
    // Try the configured name field first
    if let Some(name_node) = node.child_by_field_name(grammar.name_field) {
        let name = get_node_text(&name_node, source);
        if !name.is_empty() {
            return Some(name);
        }
    }

    // Fallback: look for identifier child
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            let name = get_node_text(&child, source);
            if !name.is_empty() {
                return Some(name);
            }
        }
    }

    None
}

fn calculate_symbol_score(
    name: &str,
    kind: &SymbolKind,
    is_exported: bool,
    filename_stem: &str,
    grammar: &LangGrammar,
) -> i32 {
    let mut score = 0;

    // Base score by kind
    score += match kind {
        SymbolKind::Class => 30,
        SymbolKind::Struct => 30,
        SymbolKind::Trait => 28,
        SymbolKind::Enum => 25,
        SymbolKind::Function => 10,
        SymbolKind::Method => 15,
        _ => 5,
    };

    // Bonus for exported
    if is_exported {
        score += 50;
    }

    // Bonus for filename match
    let name_lower = name.to_lowercase();
    if name_lower == filename_stem {
        score += 40; // Exact match
    } else if name_lower.contains(filename_stem) || filename_stem.contains(&name_lower) {
        score += 20; // Partial match
    }

    // Bonus for main/Main
    if name == "main" || name == "Main" {
        score += 30;
    }

    // Penalty for test functions
    if name.starts_with("test") || name.starts_with("Test") || name.starts_with("_test") {
        score -= 30;
    }

    // Go-specific: uppercase bonus already handled by is_exported
    if grammar.uppercase_is_export && !is_exported && name.chars().next().map(|c| c.is_lowercase()).unwrap_or(true) {
        score -= 10;
    }

    score
}

fn extract_filename_stem(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

// =============================================================================
// Import Extraction
// =============================================================================

fn extract_imports(summary: &mut SemanticSummary, root: &Node, source: &str, grammar: &LangGrammar) {
    visit_all(root, |node| {
        let kind = node.kind();
        if grammar.import_nodes.contains(&kind) {
            if let Some(import_name) = extract_import_name(node, source, grammar) {
                if !import_name.is_empty() && !summary.added_dependencies.contains(&import_name) {
                    summary.added_dependencies.push(import_name);
                }
            }
        }
    });
}

fn extract_import_name(node: &Node, source: &str, grammar: &LangGrammar) -> Option<String> {
    // Try common patterns

    // Pattern 1: path field (Go imports)
    if let Some(path_node) = node.child_by_field_name("path") {
        let path = get_node_text(&path_node, source);
        let clean = path.trim_matches('"').trim_matches('\'');
        if let Some(last) = clean.split('/').last() {
            return Some(last.to_string());
        }
    }

    // Pattern 2: source field (JS/TS imports)
    if let Some(source_node) = node.child_by_field_name("source") {
        let path = get_node_text(&source_node, source);
        let clean = path.trim_matches('"').trim_matches('\'');
        if let Some(last) = clean.split('/').last() {
            return Some(last.to_string());
        }
    }

    // Pattern 3: module_name field (Python imports)
    if let Some(module) = node.child_by_field_name("module_name") {
        return Some(get_node_text(&module, source));
    }
    if let Some(name) = node.child_by_field_name("name") {
        return Some(get_node_text(&name, source));
    }

    // Pattern 4: argument field (Rust use declarations)
    if let Some(arg) = node.child_by_field_name("argument") {
        let text = get_node_text_normalized(&arg, source);
        // Extract the first path segment
        if let Some(first) = text.split("::").next() {
            return Some(first.trim().to_string());
        }
    }

    // Pattern 5: C/C++ includes
    if node.kind() == "preproc_include" {
        if let Some(path) = node.child_by_field_name("path") {
            let include = get_node_text(&path, source);
            let clean = include.trim_matches('"').trim_matches('<').trim_matches('>');
            return Some(clean.to_string());
        }
    }

    // Fallback: get the whole node text and extract something useful
    let text = get_node_text(node, source);
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() > 1 {
        // Skip "import", "use", "from", "#include"
        if let Some(last) = words.last() {
            let clean = last.trim_matches(|c| c == '"' || c == '\'' || c == ';' || c == '<' || c == '>');
            if !clean.is_empty() {
                return Some(clean.to_string());
            }
        }
    }

    None
}

// =============================================================================
// State Change Extraction
// =============================================================================

fn extract_state_changes(
    summary: &mut SemanticSummary,
    root: &Node,
    source: &str,
    grammar: &LangGrammar,
) {
    visit_all(root, |node| {
        let kind = node.kind();

        // Variable declarations
        if grammar.var_declaration_nodes.contains(&kind) {
            if let Some(state_change) = extract_var_declaration(node, source, grammar) {
                summary.state_changes.push(state_change);
            }
        }

        // Assignments
        if grammar.assignment_nodes.contains(&kind) {
            if let Some(state_change) = extract_assignment(node, source, grammar) {
                summary.state_changes.push(state_change);
            }
        }
    });
}

fn extract_var_declaration(node: &Node, source: &str, grammar: &LangGrammar) -> Option<StateChange> {
    // Try to get name from various fields
    let name = node
        .child_by_field_name("name")
        .or_else(|| node.child_by_field_name("declarator"))
        .or_else(|| node.child_by_field_name("left"))
        .or_else(|| find_identifier_child(node))
        .map(|n| get_node_text(&n, source))?;

    if name.is_empty() {
        return None;
    }

    // Try to get type
    let state_type = node
        .child_by_field_name(grammar.type_field)
        .or_else(|| node.child_by_field_name("type"))
        .map(|n| get_node_text_normalized(&n, source))
        .unwrap_or_else(|| "_".to_string());

    // Try to get initializer
    let initializer = node
        .child_by_field_name(grammar.value_field)
        .or_else(|| node.child_by_field_name("value"))
        .or_else(|| node.child_by_field_name("right"))
        .map(|n| compress_initializer(&get_node_text_normalized(&n, source)))
        .unwrap_or_default();

    Some(StateChange {
        name,
        state_type,
        initializer,
    })
}

fn extract_assignment(node: &Node, source: &str, grammar: &LangGrammar) -> Option<StateChange> {
    let left = node.child_by_field_name("left")?;
    let right = node.child_by_field_name(grammar.value_field)
        .or_else(|| node.child_by_field_name("right"))?;

    let name = get_node_text(&left, source);
    if name.is_empty() {
        return None;
    }

    let initializer = compress_initializer(&get_node_text_normalized(&right, source));

    Some(StateChange {
        name,
        state_type: "_".to_string(),
        initializer,
    })
}

fn find_identifier_child<'a>(node: &'a Node<'a>) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "identifier" || child.kind() == "variable_declarator" {
            return Some(child);
        }
    }
    None
}

fn compress_initializer(init: &str) -> String {
    // Use the common utility if available, otherwise simple truncation
    if init.len() <= 60 {
        init.to_string()
    } else {
        format!("{}...", &init[..57])
    }
}

// =============================================================================
// Control Flow Extraction
// =============================================================================

fn extract_control_flow(
    summary: &mut SemanticSummary,
    root: &Node,
    _source: &str,
    grammar: &LangGrammar,
) {
    let mut results: Vec<ControlFlowChange> = Vec::new();
    collect_control_flow_recursive(root, 0, grammar, &mut results);
    summary.control_flow_changes.extend(results);
}

fn collect_control_flow_recursive(
    node: &Node,
    depth: usize,
    grammar: &LangGrammar,
    results: &mut Vec<ControlFlowChange>,
) {
    let kind = node.kind();

    if grammar.control_flow_nodes.contains(&kind) || grammar.try_nodes.contains(&kind) {
        let cf_kind = map_control_flow_kind(kind, grammar);

        let location = Location::new(
            node.start_position().row + 1,
            node.start_position().column,
        );

        results.push(ControlFlowChange {
            kind: cf_kind,
            location,
            nesting_depth: depth,
        });
    }

    let is_control_flow =
        grammar.control_flow_nodes.contains(&kind) || grammar.try_nodes.contains(&kind);
    let new_depth = if is_control_flow { depth + 1 } else { depth };

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_control_flow_recursive(&child, new_depth, grammar, results);
    }
}

fn map_control_flow_kind(node_kind: &str, grammar: &LangGrammar) -> ControlFlowKind {
    // Check try nodes first
    if grammar.try_nodes.contains(&node_kind) {
        return ControlFlowKind::Try;
    }

    // Map based on node name patterns
    if node_kind.contains("if") {
        ControlFlowKind::If
    } else if node_kind.contains("for") || node_kind.contains("loop") {
        ControlFlowKind::For
    } else if node_kind.contains("while") {
        ControlFlowKind::While
    } else if node_kind.contains("match") || node_kind.contains("switch") {
        ControlFlowKind::Match
    } else if node_kind.contains("try") {
        ControlFlowKind::Try
    } else if node_kind.contains("with") {
        ControlFlowKind::Try // 'with' is like a context manager
    } else {
        ControlFlowKind::If // Default fallback
    }
}


// =============================================================================
// Call Extraction
// =============================================================================

fn extract_calls(summary: &mut SemanticSummary, root: &Node, source: &str, grammar: &LangGrammar) {
    let mut seen_calls: std::collections::HashSet<String> = std::collections::HashSet::new();

    visit_all(root, |node| {
        let kind = node.kind();

        if grammar.call_nodes.contains(&kind) {
            if let Some(call) = extract_call(node, source, grammar) {
                // Deduplicate calls
                let key = format!("{}:{}", call.name, call.is_awaited);
                if !seen_calls.contains(&key) {
                    seen_calls.insert(key);
                    summary.calls.push(call);
                }
            }
        }
    });
}

fn extract_call(node: &Node, source: &str, grammar: &LangGrammar) -> Option<Call> {
    // Get the function name
    let func_node = node
        .child_by_field_name("function")
        .or_else(|| node.child_by_field_name("name"))
        .or_else(|| node.child(0))?;

    let full_name = get_node_text(&func_node, source);
    if full_name.is_empty() || full_name.len() > 100 {
        return None;
    }

    // Split into object and method for method calls (e.g., "console.log" -> object="console", name="log")
    let (object, name) = if full_name.contains('.') {
        let parts: Vec<&str> = full_name.rsplitn(2, '.').collect();
        if parts.len() == 2 {
            (Some(parts[1].to_string()), parts[0].to_string())
        } else {
            (None, full_name)
        }
    } else {
        (None, full_name)
    };

    // Check if this is an async call (inside await)
    let is_awaited = if let Some(parent) = node.parent() {
        grammar.await_nodes.contains(&parent.kind())
    } else {
        false
    };

    // Check if this is inside a try block
    let in_try = is_inside_try(node, grammar);

    // Check if this is a React hook
    let is_hook = Call::check_is_hook(&name);

    // Check if this is an I/O operation
    let is_io = Call::check_is_io(&name);

    let location = Location::new(
        node.start_position().row + 1,
        node.start_position().column,
    );

    Some(Call {
        name,
        object,
        is_awaited,
        in_try,
        is_hook,
        is_io,
        location,
    })
}

fn is_inside_try(node: &Node, grammar: &LangGrammar) -> bool {
    let mut current = node.parent();
    while let Some(parent) = current {
        if grammar.try_nodes.contains(&parent.kind()) {
            return true;
        }
        current = parent.parent();
    }
    false
}

// =============================================================================
// Complexity and Risk Calculation
// =============================================================================

fn calculate_complexity(summary: &mut SemanticSummary) {
    // Cognitive complexity is calculated from control flow changes
    // This affects the behavioral_risk level
}

fn determine_risk(summary: &mut SemanticSummary) {
    // Calculate cognitive complexity from control flow
    let mut complexity: usize = 0;
    let mut max_depth: usize = 0;

    for cf in &summary.control_flow_changes {
        // Base complexity for each control flow construct
        complexity += 1;

        // Nesting penalty
        complexity += cf.nesting_depth;

        // Track max depth
        if cf.nesting_depth > max_depth {
            max_depth = cf.nesting_depth;
        }

        // Extra penalty for complex constructs
        match cf.kind {
            ControlFlowKind::Match => complexity += 1,
            ControlFlowKind::Try => complexity += 1,
            _ => {}
        }
    }

    let state_count = summary.state_changes.len();
    let call_count = summary.calls.len();

    // Risk scoring
    let risk_score = complexity / 5 + max_depth * 2 + state_count / 10 + call_count / 20;

    summary.behavioral_risk = if risk_score > 20 {
        RiskLevel::High
    } else if risk_score > 8 {
        RiskLevel::Medium
    } else {
        RiskLevel::Low
    };
}

// =============================================================================
// Utility Functions
// =============================================================================

fn visit_all<F>(node: &Node, mut callback: F)
where
    F: FnMut(&Node),
{
    visit_all_recursive(node, &mut callback);
}

fn visit_all_recursive<F>(node: &Node, callback: &mut F)
where
    F: FnMut(&Node),
{
    callback(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_all_recursive(&child, callback);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_filename_stem() {
        assert_eq!(extract_filename_stem("/path/to/main.rs"), "main");
        assert_eq!(extract_filename_stem("server.go"), "server");
        assert_eq!(extract_filename_stem("MyClass.java"), "myclass");
    }

    #[test]
    fn test_compress_initializer() {
        assert_eq!(compress_initializer("simple"), "simple");
        // Input: 65 chars, truncated to first 57 chars + "..."
        assert_eq!(
            compress_initializer("this is a very long initializer that should be truncated to fit"),
            "this is a very long initializer that should be truncated ..."
        );
    }
}
