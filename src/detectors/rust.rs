//! Rust language detector
//!
//! Extracts semantic information from Rust source files including:
//! - Primary symbol detection with improved heuristics
//! - Use statements (dependencies)
//! - Let bindings (state changes)
//! - Control flow patterns

use tree_sitter::{Node, Tree};

use crate::detectors::common::{
    compress_initializer, get_node_text, get_node_text_normalized, visit_all,
};
use crate::error::Result;
use crate::schema::{
    ControlFlowChange, ControlFlowKind, Location, RiskLevel, SemanticSummary, StateChange,
    SymbolInfo, SymbolKind,
};

/// Extract semantic information from a Rust source file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol with improved heuristics
    find_primary_symbol(summary, &root, source);

    // Extract use statements
    extract_imports(summary, &root, source);

    // Extract let bindings
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
    start_line: usize,
    end_line: usize,
    score: i32,
}

/// Find all symbols in a Rust file and populate both primary and symbols vec
///
/// Priority order (per Priority 3.0F):
/// 1. Public structs/enums/traits (highest priority for types)
/// 2. Public functions that match filename
/// 3. Other public functions
/// 4. Private structs/enums/traits
/// 5. Private functions
fn find_primary_symbol(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut candidates: Vec<SymbolCandidate> = Vec::new();

    // Extract filename stem for matching (e.g., "toon.rs" -> "toon")
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
        let (name, kind) = match child.kind() {
            "struct_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    (get_node_text(&name_node, source), SymbolKind::Struct)
                } else {
                    continue;
                }
            }
            "enum_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    (get_node_text(&name_node, source), SymbolKind::Enum)
                } else {
                    continue;
                }
            }
            "trait_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    (get_node_text(&name_node, source), SymbolKind::Trait)
                } else {
                    continue;
                }
            }
            "impl_item" => {
                if let Some(type_node) = child.child_by_field_name("type") {
                    (get_node_text(&type_node, source), SymbolKind::Method)
                } else {
                    continue;
                }
            }
            "function_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    (get_node_text(&name_node, source), SymbolKind::Function)
                } else {
                    continue;
                }
            }
            "mod_item" => {
                // Skip inline module definitions for now
                continue;
            }
            _ => continue,
        };

        let is_public = has_pub_visibility(&child, source);
        let score = calculate_symbol_score(&name, &kind, is_public, filename_stem);

        candidates.push(SymbolCandidate {
            name,
            kind,
            is_public,
            start_line: child.start_position().row + 1,
            end_line: child.end_position().row + 1,
            score,
        });
    }
}

/// Calculate a score for symbol prioritization
///
/// Higher scores = better candidates for primary symbol
fn calculate_symbol_score(name: &str, kind: &SymbolKind, is_public: bool, filename_stem: &str) -> i32 {
    let mut score = 0;

    // Base score by kind (types preferred over functions)
    score += match kind {
        SymbolKind::Struct => 30,
        SymbolKind::Enum => 28,
        SymbolKind::Trait => 26,
        SymbolKind::Method => 20, // impl block
        SymbolKind::Function => 10,
        _ => 5,
    };

    // Bonus for public visibility
    if is_public {
        score += 50;
    }

    // Bonus for filename match
    let name_lower = name.to_lowercase();
    if name_lower == filename_stem {
        // Exact match (e.g., toon.rs contains Toon)
        score += 40;
    } else if name_lower.contains(filename_stem) || filename_stem.contains(&name_lower) {
        // Partial match (e.g., toon.rs contains encode_toon)
        score += 20;
    }

    // Penalty for test/helper functions
    if name.starts_with("test_") || name.starts_with("_") {
        score -= 30;
    }
    if name.contains("helper") || name.contains("util") {
        score -= 10;
    }

    // Bonus for common primary symbol patterns
    if name == "main" || name == "run" || name == "execute" {
        score += 15;
    }
    if name.starts_with("new") || name == "default" {
        // These are constructors, not primary symbols
        score -= 20;
    }

    score
}

/// Check if a node has pub visibility
fn has_pub_visibility(node: &Node, source: &str) -> bool {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "visibility_modifier" {
            let text = get_node_text(&child, source);
            // Match "pub", "pub(crate)", "pub(super)", "pub(in path)"
            return text.starts_with("pub");
        }
    }
    false
}

// ============================================================================
// Import Extraction
// ============================================================================

/// Extract use statements as dependencies
pub fn extract_imports(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "use_declaration" {
            // Get the full use path
            if let Some(arg) = child.child_by_field_name("argument") {
                let use_text = get_node_text_normalized(&arg, source);
                // Extract the last segment as the imported name
                if let Some(last) = use_text.split("::").last() {
                    // Clean up braces and normalize the import names
                    let cleaned = last.trim_matches('{').trim_matches('}').trim();
                    // Split comma-separated imports in a use group
                    for name in cleaned.split(',') {
                        let name = name.trim();
                        if !name.is_empty() && name != "*" {
                            summary.added_dependencies.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
}

// ============================================================================
// State Extraction
// ============================================================================

/// Extract let bindings as state changes
pub fn extract_state(summary: &mut SemanticSummary, root: &Node, source: &str) {
    visit_all(root, |node| {
        if node.kind() == "let_declaration" {
            if let Some(pattern) = node.child_by_field_name("pattern") {
                let name = get_node_text_normalized(&pattern, source);
                let type_str = node
                    .child_by_field_name("type")
                    .map(|t| get_node_text_normalized(&t, source))
                    .unwrap_or_else(|| "_".to_string());
                let init = node
                    .child_by_field_name("value")
                    .map(|v| compress_initializer(&get_node_text(&v, source)))
                    .unwrap_or_else(|| "_".to_string());

                summary.state_changes.push(StateChange {
                    name,
                    state_type: type_str,
                    initializer: init,
                });
            }
        }
    });
}

// ============================================================================
// Control Flow Extraction
// ============================================================================

/// Extract control flow patterns
pub fn extract_control_flow(summary: &mut SemanticSummary, root: &Node) {
    visit_all(root, |node| {
        let kind = match node.kind() {
            "if_expression" => Some(ControlFlowKind::If),
            "for_expression" => Some(ControlFlowKind::For),
            "while_expression" => Some(ControlFlowKind::While),
            "match_expression" => Some(ControlFlowKind::Match),
            "loop_expression" => Some(ControlFlowKind::Loop),
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
        assert_eq!(extract_filename_stem("/path/to/toon.rs"), "toon");
        assert_eq!(extract_filename_stem("lib.rs"), "lib");
        assert_eq!(extract_filename_stem("mod.rs"), "mod");
    }

    #[test]
    fn test_calculate_symbol_score() {
        // Public struct matching filename should score highest
        let pub_struct_match = calculate_symbol_score("Toon", &SymbolKind::Struct, true, "toon");
        let priv_func = calculate_symbol_score("helper", &SymbolKind::Function, false, "toon");
        assert!(pub_struct_match > priv_func, "pub struct with match should beat private func");

        // Public struct with exact filename match should beat function with partial match
        let pub_func_partial = calculate_symbol_score("encode_toon", &SymbolKind::Function, true, "toon");
        let pub_struct_exact = calculate_symbol_score("Toon", &SymbolKind::Struct, true, "toon");
        assert!(pub_struct_exact > pub_func_partial, "exact match struct beats partial match func");

        // Test helpers get penalized
        let helper = calculate_symbol_score("helper_func", &SymbolKind::Function, true, "toon");
        let normal = calculate_symbol_score("process", &SymbolKind::Function, true, "toon");
        assert!(normal > helper, "helpers should be penalized");

        // Test _ prefix penalty
        let private = calculate_symbol_score("_internal", &SymbolKind::Function, true, "toon");
        assert!(normal > private, "underscore prefix should be penalized");
    }
}
