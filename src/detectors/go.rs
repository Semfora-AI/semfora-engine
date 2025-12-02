//! Go language detector
//!
//! Extracts semantic information from Go source files including:
//! - Primary symbol detection with improved heuristics
//! - Import statements (dependencies)
//! - Type declarations and functions

use tree_sitter::{Node, Tree};

use crate::detectors::common::{get_node_text, visit_all};
use crate::error::Result;
use crate::schema::{SemanticSummary, SymbolKind};

/// Extract semantic information from a Go source file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol with improved heuristics
    find_primary_symbol(summary, &root, source);

    // Extract imports
    extract_imports(summary, &root, source);

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
    is_exported: bool,
    start_line: usize,
    end_line: usize,
    score: i32,
}

/// Find the primary symbol in a Go file with improved heuristics
///
/// Go convention: exported names start with uppercase
/// Priority order:
/// 1. Exported structs/interfaces matching filename
/// 2. Other exported types
/// 3. Exported functions matching filename
/// 4. Main function (for main packages)
/// 5. Other exported functions
fn find_primary_symbol(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut candidates: Vec<SymbolCandidate> = Vec::new();

    // Extract filename stem for matching
    let filename_stem = extract_filename_stem(&summary.file);

    collect_symbol_candidates(root, source, &filename_stem, &mut candidates);

    // Sort by score (highest first)
    candidates.sort_by(|a, b| b.score.cmp(&a.score));

    // Use the best candidate
    if let Some(best) = candidates.first() {
        summary.symbol = Some(best.name.clone());
        summary.symbol_kind = Some(best.kind.clone());
        summary.start_line = Some(best.start_line);
        summary.end_line = Some(best.end_line);
        summary.public_surface_changed = best.is_exported;
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
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = get_node_text(&name_node, source);
                    let is_exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    let score = calculate_symbol_score(&name, &SymbolKind::Function, is_exported, filename_stem);

                    candidates.push(SymbolCandidate {
                        name,
                        kind: SymbolKind::Function,
                        is_exported,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        score,
                    });
                }
            }
            "method_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = get_node_text(&name_node, source);
                    let is_exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);
                    let score = calculate_symbol_score(&name, &SymbolKind::Method, is_exported, filename_stem);

                    candidates.push(SymbolCandidate {
                        name,
                        kind: SymbolKind::Method,
                        is_exported,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        score,
                    });
                }
            }
            "type_declaration" => {
                // Look for struct or interface type specs
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "type_spec" {
                        if let Some(name_node) = inner.child_by_field_name("name") {
                            let name = get_node_text(&name_node, source);
                            let is_exported = name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false);

                            // Determine if it's a struct or interface
                            let kind = determine_type_kind(&inner);
                            let score = calculate_symbol_score(&name, &kind, is_exported, filename_stem);

                            candidates.push(SymbolCandidate {
                                name,
                                kind,
                                is_exported,
                                start_line: child.start_position().row + 1,
                                end_line: child.end_position().row + 1,
                                score,
                            });
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

/// Determine if a type_spec is a struct, interface, or other type
fn determine_type_kind(type_spec: &Node) -> SymbolKind {
    if let Some(type_node) = type_spec.child_by_field_name("type") {
        match type_node.kind() {
            "struct_type" => return SymbolKind::Struct,
            "interface_type" => return SymbolKind::Trait, // Use Trait for interfaces
            _ => {}
        }
    }
    SymbolKind::Struct // Default to struct for type aliases
}

/// Calculate a score for symbol prioritization
fn calculate_symbol_score(
    name: &str,
    kind: &SymbolKind,
    is_exported: bool,
    filename_stem: &str,
) -> i32 {
    let mut score = 0;

    // Base score by kind (types preferred over functions)
    score += match kind {
        SymbolKind::Struct => 30,
        SymbolKind::Trait => 28, // interface
        SymbolKind::Method => 15,
        SymbolKind::Function => 10,
        _ => 5,
    };

    // Bonus for exported (uppercase start)
    if is_exported {
        score += 50;
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

    // Bonus for main function in main.go
    if name == "main" && filename_stem == "main" {
        score += 30;
    }

    // Penalty for test functions
    if name.starts_with("Test") || name.starts_with("Benchmark") {
        score -= 30;
    }

    score
}

// ============================================================================
// Import Extraction
// ============================================================================

/// Extract import statements as dependencies
pub fn extract_imports(summary: &mut SemanticSummary, root: &Node, source: &str) {
    visit_all(root, |node| {
        if node.kind() == "import_spec" {
            if let Some(path) = node.child_by_field_name("path") {
                let import_path = get_node_text(&path, source);
                // Get the last segment of the import path
                let clean = import_path.trim_matches('"');
                if let Some(last) = clean.split('/').last() {
                    summary.added_dependencies.push(last.to_string());
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_filename_stem() {
        assert_eq!(extract_filename_stem("/path/to/server.go"), "server");
        assert_eq!(extract_filename_stem("main.go"), "main");
        assert_eq!(extract_filename_stem("handler_test.go"), "handler_test");
    }

    #[test]
    fn test_calculate_symbol_score() {
        // Exported struct should beat unexported function
        let exported_struct = calculate_symbol_score("Server", &SymbolKind::Struct, true, "server");
        let unexported_func = calculate_symbol_score("helper", &SymbolKind::Function, false, "server");
        assert!(exported_struct > unexported_func);

        // main function in main.go gets bonus
        let main_fn = calculate_symbol_score("main", &SymbolKind::Function, false, "main");
        let other_fn = calculate_symbol_score("helper", &SymbolKind::Function, false, "main");
        assert!(main_fn > other_fn);

        // Test functions should be penalized
        let test_fn = calculate_symbol_score("TestServer", &SymbolKind::Function, true, "server");
        let normal_fn = calculate_symbol_score("CreateServer", &SymbolKind::Function, true, "server");
        assert!(normal_fn > test_fn);
    }
}
