//! Python language detector
//!
//! Extracts semantic information from Python source files including:
//! - Primary symbol detection with improved heuristics
//! - Import statements (dependencies)
//! - Variable assignments (state changes)
//! - Control flow patterns

use tree_sitter::{Node, Tree};

use crate::detectors::common::{get_node_text, visit_all};
use crate::error::Result;
use crate::schema::{
    ControlFlowChange, ControlFlowKind, Location, RiskLevel, SemanticSummary, StateChange,
    SymbolInfo, SymbolKind,
};

/// Extract semantic information from a Python source file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol with improved heuristics
    find_primary_symbol(summary, &root, source);

    // Extract imports
    extract_imports(summary, &root, source);

    // Extract variable assignments
    extract_state(summary, &root, source);

    // Extract control flow
    extract_control_flow(summary, &root);

    Ok(())
}

// ============================================================================
// Symbol Detection with Improved Heuristics
// ============================================================================

/// Candidate symbol for ranking
#[derive(Debug)]
struct SymbolCandidate {
    name: String,
    kind: SymbolKind,
    is_public: bool,
    is_decorated: bool,
    start_line: usize,
    end_line: usize,
    score: i32,
}

/// Find all symbols in a Python file and populate both primary and symbols vec
///
/// Priority order:
/// 1. Public classes (especially with decorators like @dataclass)
/// 2. Public functions matching filename
/// 3. Other public functions
/// 4. Private classes/functions (starting with _)
fn find_primary_symbol(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut candidates: Vec<SymbolCandidate> = Vec::new();

    // Extract filename stem for matching
    let filename_stem = extract_filename_stem(&summary.file);

    collect_symbol_candidates(root, source, &filename_stem, &mut candidates);

    // Sort by score (highest first)
    candidates.sort_by(|a, b| b.score.cmp(&a.score));

    // Convert ALL public candidates to SymbolInfo and add to summary.symbols
    for candidate in &candidates {
        if candidate.is_public || candidate.score > 0 {
            let symbol_info = SymbolInfo {
                name: candidate.name.clone(),
                kind: candidate.kind,
                start_line: candidate.start_line,
                end_line: candidate.end_line,
                is_exported: candidate.is_public,
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
    }

    // Use the best candidate for primary symbol (backward compatibility)
    if let Some(best) = candidates.first() {
        summary.symbol = Some(best.name.clone());
        summary.symbol_kind = Some(best.kind);
        summary.start_line = Some(best.start_line);
        summary.end_line = Some(best.end_line);
        summary.public_surface_changed = best.is_public;
    }
}

/// Extract the filename stem from a file path
fn extract_filename_stem(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// Collect all symbol candidates from the AST
fn collect_symbol_candidates(
    root: &Node,
    source: &str,
    filename_stem: &str,
    candidates: &mut Vec<SymbolCandidate>,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = get_node_text(&name_node, source);
                    let is_public = !name.starts_with('_');
                    let score = calculate_symbol_score(&name, &SymbolKind::Function, is_public, false, filename_stem);

                    candidates.push(SymbolCandidate {
                        name,
                        kind: SymbolKind::Function,
                        is_public,
                        is_decorated: false,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        score,
                    });
                }
            }
            "class_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = get_node_text(&name_node, source);
                    let is_public = !name.starts_with('_');
                    let score = calculate_symbol_score(&name, &SymbolKind::Class, is_public, false, filename_stem);

                    candidates.push(SymbolCandidate {
                        name,
                        kind: SymbolKind::Class,
                        is_public,
                        is_decorated: false,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        score,
                    });
                }
            }
            "decorated_definition" => {
                // Look inside decorated definition
                let has_special_decorator = has_important_decorator(&child, source);
                let mut inner_cursor = child.walk();

                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "function_definition" => {
                            if let Some(name_node) = inner.child_by_field_name("name") {
                                let name = get_node_text(&name_node, source);
                                let is_public = !name.starts_with('_');
                                let score = calculate_symbol_score(&name, &SymbolKind::Function, is_public, has_special_decorator, filename_stem);

                                candidates.push(SymbolCandidate {
                                    name,
                                    kind: SymbolKind::Function,
                                    is_public,
                                    is_decorated: has_special_decorator,
                                    start_line: child.start_position().row + 1, // Use outer span
                                    end_line: child.end_position().row + 1,
                                    score,
                                });
                            }
                        }
                        "class_definition" => {
                            if let Some(name_node) = inner.child_by_field_name("name") {
                                let name = get_node_text(&name_node, source);
                                let is_public = !name.starts_with('_');
                                let score = calculate_symbol_score(&name, &SymbolKind::Class, is_public, has_special_decorator, filename_stem);

                                candidates.push(SymbolCandidate {
                                    name,
                                    kind: SymbolKind::Class,
                                    is_public,
                                    is_decorated: has_special_decorator,
                                    start_line: child.start_position().row + 1, // Use outer span
                                    end_line: child.end_position().row + 1,
                                    score,
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

/// Check if a decorated definition has important decorators
fn has_important_decorator(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "decorator" {
            let text = get_node_text(&child, source).to_lowercase();
            // Important decorators that indicate primary symbols
            if text.contains("dataclass")
                || text.contains("app.route")
                || text.contains("router")
                || text.contains("api")
                || text.contains("endpoint")
                || text.contains("pytest")
                || text.contains("fixture")
            {
                return true;
            }
        }
    }
    false
}

/// Calculate a score for symbol prioritization
fn calculate_symbol_score(
    name: &str,
    kind: &SymbolKind,
    is_public: bool,
    is_decorated: bool,
    filename_stem: &str,
) -> i32 {
    let mut score = 0;

    // Base score by kind (classes preferred over functions)
    score += match kind {
        SymbolKind::Class => 30,
        SymbolKind::Function => 10,
        _ => 5,
    };

    // Bonus for public (not starting with _)
    if is_public {
        score += 50;
    }

    // Bonus for important decorators
    if is_decorated {
        score += 25;
    }

    // Bonus for filename match
    let name_lower = name.to_lowercase();
    if name_lower == filename_stem {
        // Exact match
        score += 40;
    } else if name_lower.contains(filename_stem) || filename_stem.contains(&name_lower) {
        // Partial match
        score += 20;
    }

    // Penalty for test functions
    if name.starts_with("test_") || name.starts_with("Test") {
        score -= 30;
    }

    // Penalty for dunder methods
    if name.starts_with("__") && name.ends_with("__") {
        score -= 40;
    }

    // Bonus for common entry points
    if name == "main" || name == "run" || name == "app" {
        score += 15;
    }

    score
}

// ============================================================================
// Import Extraction
// ============================================================================

/// Extract import statements as dependencies
pub fn extract_imports(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                if let Some(name) = child.child_by_field_name("name") {
                    summary.added_dependencies.push(get_node_text(&name, source));
                }
            }
            "import_from_statement" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "dotted_name" || inner.kind() == "aliased_import" {
                        summary.added_dependencies.push(get_node_text(&inner, source));
                    }
                }
            }
            _ => {}
        }
    }
}

// ============================================================================
// State Extraction
// ============================================================================

/// Extract variable assignments as state changes
pub fn extract_state(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "assignment" {
                    if let Some(left) = inner.child_by_field_name("left") {
                        if let Some(right) = inner.child_by_field_name("right") {
                            summary.state_changes.push(StateChange {
                                name: get_node_text(&left, source),
                                state_type: "_".to_string(),
                                initializer: get_node_text(&right, source),
                            });
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// Control Flow Extraction
// ============================================================================

/// Extract control flow patterns
pub fn extract_control_flow(summary: &mut SemanticSummary, root: &Node) {
    visit_all(root, |node| {
        let kind = match node.kind() {
            "if_statement" => Some(ControlFlowKind::If),
            "for_statement" => Some(ControlFlowKind::For),
            "while_statement" => Some(ControlFlowKind::While),
            "try_statement" => Some(ControlFlowKind::Try),
            "match_statement" => Some(ControlFlowKind::Match),
            _ => None,
        };

        if let Some(k) = kind {
            summary.control_flow_changes.push(ControlFlowChange {
                kind: k,
                location: Location::new(
                    node.start_position().row + 1,
                    node.start_position().column,
                ),
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_filename_stem() {
        assert_eq!(extract_filename_stem("/path/to/models.py"), "models");
        assert_eq!(extract_filename_stem("utils.py"), "utils");
        assert_eq!(extract_filename_stem("__init__.py"), "__init__");
    }

    #[test]
    fn test_calculate_symbol_score() {
        // Public class should beat private function
        let pub_class = calculate_symbol_score("User", &SymbolKind::Class, true, false, "user");
        let priv_func = calculate_symbol_score("_helper", &SymbolKind::Function, false, false, "user");
        assert!(pub_class > priv_func);

        // Decorated class should get bonus
        let decorated = calculate_symbol_score("Config", &SymbolKind::Class, true, true, "config");
        let plain = calculate_symbol_score("Config", &SymbolKind::Class, true, false, "config");
        assert!(decorated > plain);

        // Test functions should be penalized
        let test_func = calculate_symbol_score("test_user", &SymbolKind::Function, true, false, "user");
        let normal_func = calculate_symbol_score("create_user", &SymbolKind::Function, true, false, "user");
        assert!(normal_func > test_func);
    }
}
