//! JavaScript/TypeScript/JSX/TSX language detector
//!
//! Extracts semantic information from JavaScript family files including:
//! - Primary symbol detection with improved heuristics (exports prioritized)
//! - Import statements (dependencies and local imports)
//! - State hooks (useState, useReducer)
//! - JSX elements and component detection
//! - Control flow patterns
//! - Function calls with context (awaited, in try block)

use tree_sitter::{Node, Tree};

use crate::detectors::common::{get_node_text, push_unique_insertion, visit_all};
use crate::error::Result;
use crate::lang::Lang;
use crate::schema::{
    Argument, Call, ControlFlowChange, ControlFlowKind, Location, Prop, SemanticSummary,
    StateChange, SymbolKind,
};
use crate::toon::is_meaningful_call;

/// Extract semantic information from a JavaScript/TypeScript file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree, lang: Lang) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol with improved heuristics
    find_primary_symbol(summary, &root, source);

    // Extract imports
    extract_imports(summary, &root, source);

    // Extract state hooks (useState, useReducer)
    extract_state_hooks(summary, &root, source);

    // Extract JSX elements for insertion rules
    if lang.supports_jsx() {
        extract_jsx_insertions(summary, &root, source);
    }

    // Extract control flow
    extract_control_flow(summary, &root);

    // Extract function calls with context
    extract_calls(summary, &root, source);

    // Generate semantic insertions based on file context
    generate_insertions(summary, source);

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
    is_default_export: bool,
    returns_jsx: bool,
    start_line: usize,
    end_line: usize,
    arguments: Vec<Argument>,
    props: Vec<Prop>,
    score: i32,
}

/// Find the primary symbol with improved heuristics
///
/// Priority order:
/// 1. Default exported components (function returning JSX)
/// 2. Named exported components
/// 3. Default exported functions/classes
/// 4. Named exported functions/classes
/// 5. Non-exported functions/classes (file-local)
fn find_primary_symbol(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut candidates: Vec<SymbolCandidate> = Vec::new();
    let filename_stem = extract_filename_stem(&summary.file);

    collect_symbol_candidates(root, source, &filename_stem, &mut candidates);

    // Sort by score (highest first)
    candidates.sort_by(|a, b| b.score.cmp(&a.score));

    // Use the best candidate
    if let Some(best) = candidates.into_iter().next() {
        summary.symbol = Some(best.name);
        summary.symbol_kind = Some(if best.returns_jsx {
            SymbolKind::Component
        } else {
            best.kind
        });
        summary.start_line = Some(best.start_line);
        summary.end_line = Some(best.end_line);
        summary.public_surface_changed = best.is_exported;
        summary.arguments = best.arguments;
        summary.props = best.props;

        if best.returns_jsx {
            summary.return_type = Some("JSX.Element".to_string());
        }
    }
}

/// Extract filename stem
fn extract_filename_stem(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase()
}

/// Collect symbol candidates from the AST
fn collect_symbol_candidates(
    root: &Node,
    source: &str,
    filename_stem: &str,
    candidates: &mut Vec<SymbolCandidate>,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "export_statement" => {
                let is_default = has_default_keyword(&child, source);

                // Check for declaration inside export
                if let Some(decl) = child.child_by_field_name("declaration") {
                    if let Some(mut candidate) =
                        extract_candidate_from_declaration(&decl, source, filename_stem)
                    {
                        candidate.is_exported = true;
                        candidate.is_default_export = is_default;
                        candidate.score = calculate_symbol_score(&candidate, filename_stem);
                        candidates.push(candidate);
                    }
                } else {
                    // Direct function/class inside export
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "function_declaration"
                            || inner.kind() == "class_declaration"
                        {
                            if let Some(mut candidate) =
                                extract_candidate_from_declaration(&inner, source, filename_stem)
                            {
                                candidate.is_exported = true;
                                candidate.is_default_export = is_default;
                                candidate.score = calculate_symbol_score(&candidate, filename_stem);
                                candidates.push(candidate);
                            }
                            break;
                        }
                    }
                }
            }
            "function_declaration" | "class_declaration" | "lexical_declaration" => {
                if let Some(mut candidate) =
                    extract_candidate_from_declaration(&child, source, filename_stem)
                {
                    candidate.score = calculate_symbol_score(&candidate, filename_stem);
                    candidates.push(candidate);
                }
            }
            _ => {}
        }
    }
}

/// Check if export has default keyword
fn has_default_keyword(node: &Node, source: &str) -> bool {
    let text = get_node_text(node, source);
    text.contains("export default")
}

/// Extract a symbol candidate from a declaration node
fn extract_candidate_from_declaration(
    node: &Node,
    source: &str,
    _filename_stem: &str,
) -> Option<SymbolCandidate> {
    match node.kind() {
        "function_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = get_node_text(&name_node, source);

            let mut arguments = Vec::new();
            let mut props = Vec::new();

            if let Some(params) = node.child_by_field_name("parameters") {
                extract_parameters(&params, source, &mut arguments, &mut props);
            }

            Some(SymbolCandidate {
                name,
                kind: SymbolKind::Function,
                is_exported: false,
                is_default_export: false,
                returns_jsx: returns_jsx(node),
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                arguments,
                props,
                score: 0,
            })
        }
        "class_declaration" => {
            let name_node = node.child_by_field_name("name")?;
            let name = get_node_text(&name_node, source);

            Some(SymbolCandidate {
                name,
                kind: SymbolKind::Class,
                is_exported: false,
                is_default_export: false,
                returns_jsx: false,
                start_line: node.start_position().row + 1,
                end_line: node.end_position().row + 1,
                arguments: Vec::new(),
                props: Vec::new(),
                score: 0,
            })
        }
        "lexical_declaration" => {
            // Look for arrow function assigned to const
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "variable_declarator" {
                    let name_node = child.child_by_field_name("name")?;
                    let value_node = child.child_by_field_name("value")?;

                    if value_node.kind() == "arrow_function" {
                        let name = get_node_text(&name_node, source);

                        let mut arguments = Vec::new();
                        let mut props = Vec::new();

                        if let Some(params) = value_node.child_by_field_name("parameters") {
                            extract_parameters(&params, source, &mut arguments, &mut props);
                        } else if let Some(param) = value_node.child_by_field_name("parameter") {
                            arguments.push(Argument {
                                name: get_node_text(&param, source),
                                arg_type: None,
                                default_value: None,
                            });
                        }

                        return Some(SymbolCandidate {
                            name,
                            kind: SymbolKind::Function,
                            is_exported: false,
                            is_default_export: false,
                            returns_jsx: returns_jsx(&value_node),
                            start_line: node.start_position().row + 1,
                            end_line: node.end_position().row + 1,
                            arguments,
                            props,
                            score: 0,
                        });
                    }
                }
            }
            None
        }
        _ => None,
    }
}

/// Calculate symbol score for prioritization
fn calculate_symbol_score(candidate: &SymbolCandidate, filename_stem: &str) -> i32 {
    let mut score = 0;

    // Base score by kind
    score += match candidate.kind {
        SymbolKind::Component => 40,
        SymbolKind::Class => 30,
        SymbolKind::Function => 20,
        _ => 10,
    };

    // Bonus for JSX-returning functions (components)
    if candidate.returns_jsx {
        score += 30;
    }

    // Bonus for exports
    if candidate.is_exported {
        score += 50;
    }

    // Extra bonus for default exports
    if candidate.is_default_export {
        score += 20;
    }

    // Filename matching bonus
    let name_lower = candidate.name.to_lowercase();
    if name_lower == filename_stem {
        score += 40;
    } else if name_lower.contains(filename_stem) || filename_stem.contains(&name_lower) {
        score += 20;
    }

    // Penalty for test files
    if candidate.name.starts_with("test") || candidate.name.ends_with("Test") {
        score -= 30;
    }

    // Penalty for internal/helper naming
    if candidate.name.starts_with("_") || candidate.name.contains("Helper") {
        score -= 20;
    }

    score
}

/// Check if a function returns JSX
fn returns_jsx(node: &Node) -> bool {
    contains_node_kind(node, "jsx_element")
        || contains_node_kind(node, "jsx_self_closing_element")
        || contains_node_kind(node, "jsx_fragment")
}

/// Check if a node contains a specific kind
fn contains_node_kind(node: &Node, kind: &str) -> bool {
    if node.kind() == kind {
        return true;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if contains_node_kind(&child, kind) {
            return true;
        }
    }
    false
}

/// Extract function parameters
fn extract_parameters(
    params: &Node,
    source: &str,
    arguments: &mut Vec<Argument>,
    props: &mut Vec<Prop>,
) {
    let mut cursor = params.walk();
    for child in params.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                arguments.push(Argument {
                    name: get_node_text(&child, source),
                    arg_type: None,
                    default_value: None,
                });
            }
            "required_parameter" | "optional_parameter" => {
                let name = child
                    .child_by_field_name("pattern")
                    .map(|n| get_node_text(&n, source))
                    .unwrap_or_default();
                let arg_type = child
                    .child_by_field_name("type")
                    .map(|n| get_node_text(&n, source));
                arguments.push(Argument {
                    name,
                    arg_type,
                    default_value: None,
                });
            }
            "assignment_pattern" => {
                if let Some(left) = child.child_by_field_name("left") {
                    let name = get_node_text(&left, source);
                    let default_value = child
                        .child_by_field_name("right")
                        .map(|n| get_node_text(&n, source));
                    arguments.push(Argument {
                        name,
                        arg_type: None,
                        default_value,
                    });
                }
            }
            "object_pattern" => {
                extract_object_pattern_as_props(&child, source, props);
            }
            _ => {}
        }
    }
}

/// Extract destructured props from object pattern
fn extract_object_pattern_as_props(pattern: &Node, source: &str, props: &mut Vec<Prop>) {
    let mut cursor = pattern.walk();
    for child in pattern.children(&mut cursor) {
        if child.kind() == "shorthand_property_identifier_pattern" {
            props.push(Prop {
                name: get_node_text(&child, source),
                prop_type: None,
                default_value: None,
                required: true,
            });
        } else if child.kind() == "pair_pattern" {
            if let Some(key) = child.child_by_field_name("key") {
                let name = get_node_text(&key, source);
                let default_value = child.child_by_field_name("value").and_then(|v| {
                    if v.kind() == "assignment_pattern" {
                        v.child_by_field_name("right")
                            .map(|r| get_node_text(&r, source))
                    } else {
                        None
                    }
                });
                props.push(Prop {
                    name,
                    prop_type: None,
                    default_value: default_value.clone(),
                    required: default_value.is_none(),
                });
            }
        }
    }
}

// ============================================================================
// Import Extraction
// ============================================================================

/// Extract imports as dependencies
pub fn extract_imports(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_statement" {
            if let Some(clause) = child.child_by_field_name("source") {
                let module = get_node_text(&clause, source);
                let module = module.trim_matches('"').trim_matches('\'');

                // Track local imports for data flow
                if is_local_import(module) {
                    summary.local_imports.push(normalize_import_path(module));
                }

                // Extract imported names
                extract_import_names(&child, source, module, &mut summary.added_dependencies);
            }
        }
    }
}

/// Check if an import path is local (starts with . or ..)
fn is_local_import(module: &str) -> bool {
    module.starts_with('.') || module.starts_with("..")
}

/// Normalize an import path
fn normalize_import_path(module: &str) -> String {
    module.trim_start_matches("./").to_string()
}

/// Extract imported names from import statement
fn extract_import_names(import: &Node, source: &str, module: &str, deps: &mut Vec<String>) {
    let mut cursor = import.walk();
    for child in import.children(&mut cursor) {
        match child.kind() {
            "import_clause" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    match inner.kind() {
                        "identifier" => {
                            // Default import
                            deps.push(get_node_text(&inner, source));
                        }
                        "named_imports" => {
                            let mut named_cursor = inner.walk();
                            for named in inner.children(&mut named_cursor) {
                                if named.kind() == "import_specifier" {
                                    if let Some(name_node) = named.child_by_field_name("name") {
                                        deps.push(get_node_text(&name_node, source));
                                    }
                                }
                            }
                        }
                        "namespace_import" => {
                            // import * as name
                            if let Some(name_node) = inner.child_by_field_name("name") {
                                deps.push(get_node_text(&name_node, source));
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    // If no specific imports found, use module name
    if deps.is_empty() && !module.is_empty() {
        if let Some(last) = module.split('/').last() {
            deps.push(last.to_string());
        }
    }
}

// ============================================================================
// State Hooks Extraction
// ============================================================================

/// Extract React state hooks
pub fn extract_state_hooks(summary: &mut SemanticSummary, root: &Node, source: &str) {
    visit_all(root, |node| {
        if node.kind() == "call_expression" {
            if let Some(func) = node.child_by_field_name("function") {
                let func_name = get_node_text(&func, source);
                if func_name == "useState" || func_name == "useReducer" {
                    if let Some(parent) = node.parent() {
                        if parent.kind() == "variable_declarator" {
                            if let Some(name_node) = parent.child_by_field_name("name") {
                                if name_node.kind() == "array_pattern" {
                                    let mut cursor = name_node.walk();
                                    for child in name_node.children(&mut cursor) {
                                        if child.kind() == "identifier" {
                                            let state_name = get_node_text(&child, source);

                                            let mut init = "undefined".to_string();
                                            if let Some(args) = node.child_by_field_name("arguments")
                                            {
                                                let mut arg_cursor = args.walk();
                                                for arg in args.children(&mut arg_cursor) {
                                                    if arg.kind() != "(" && arg.kind() != ")" {
                                                        init = get_node_text(&arg, source);
                                                        break;
                                                    }
                                                }
                                            }

                                            summary.state_changes.push(StateChange {
                                                name: state_name.clone(),
                                                state_type: infer_type(&init),
                                                initializer: init,
                                            });

                                            summary.insertions.push(format!(
                                                "local {} state via {}",
                                                state_name, func_name
                                            ));
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    });
}

/// Infer type from initializer
fn infer_type(init: &str) -> String {
    let trimmed = init.trim();
    if trimmed.starts_with('"') || trimmed.starts_with('\'') || trimmed.starts_with('`') {
        "string".to_string()
    } else if trimmed.parse::<i64>().is_ok() || trimmed.parse::<f64>().is_ok() {
        "number".to_string()
    } else if trimmed == "true" || trimmed == "false" {
        "boolean".to_string()
    } else if trimmed.starts_with('[') {
        "array".to_string()
    } else if trimmed.starts_with('{') {
        "object".to_string()
    } else if trimmed == "null" {
        "null".to_string()
    } else {
        "_".to_string()
    }
}

// ============================================================================
// JSX Extraction
// ============================================================================

/// Extract JSX insertions for semantic context
pub fn extract_jsx_insertions(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut jsx_tags: Vec<String> = Vec::new();
    let mut has_conditional_render = false;

    visit_all(root, |node| {
        if node.kind() == "jsx_element" || node.kind() == "jsx_self_closing_element" {
            if let Some(opening) = node.child(0) {
                let tag_node = if opening.kind() == "jsx_opening_element" {
                    opening.child_by_field_name("name")
                } else if node.kind() == "jsx_self_closing_element" {
                    node.child_by_field_name("name")
                } else {
                    None
                };

                if let Some(tag) = tag_node {
                    jsx_tags.push(get_node_text(&tag, source));
                }
            }
        }

        if node.kind() == "jsx_expression" {
            let expr_text = get_node_text(node, source);
            if expr_text.contains("&&") {
                has_conditional_render = true;
            }
        }
    });

    // Header detection
    if jsx_tags.iter().any(|t| t == "header") {
        if jsx_tags.iter().any(|t| t == "nav") {
            summary
                .insertions
                .push("header container with nav".to_string());
        } else {
            summary.insertions.push("header container".to_string());
        }
    }

    // Route links count
    let link_count = jsx_tags
        .iter()
        .filter(|t| *t == "Link" || *t == "a")
        .count();
    if link_count >= 3 {
        summary
            .insertions
            .push(format!("{} route links", link_count));
    }

    // Dropdown detection
    if jsx_tags.iter().any(|t| t == "button")
        && jsx_tags.iter().any(|t| t == "div" || t == "menu")
        && has_conditional_render
    {
        summary.insertions.push("dropdown menu".to_string());
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
            "for_statement" | "for_in_statement" => Some(ControlFlowKind::For),
            "while_statement" => Some(ControlFlowKind::While),
            "switch_statement" => Some(ControlFlowKind::Switch),
            "try_statement" => Some(ControlFlowKind::Try),
            _ => None,
        };

        if let Some(k) = kind {
            summary.control_flow_changes.push(ControlFlowChange {
                kind: k,
                location: Location::new(node.start_position().row + 1, node.start_position().column),
            });
        }
    });
}

// ============================================================================
// Call Extraction
// ============================================================================

/// Extract function calls with context
pub fn extract_calls(summary: &mut SemanticSummary, root: &Node, source: &str) {
    let mut try_ranges: Vec<(usize, usize)> = Vec::new();
    visit_all(root, |node| {
        if node.kind() == "try_statement" {
            try_ranges.push((node.start_byte(), node.end_byte()));
        }
    });

    visit_all(root, |node| {
        if node.kind() == "call_expression" {
            if let Some(func) = node.child_by_field_name("function") {
                let (name, object) = extract_call_name(&func, source);

                if Call::check_is_hook(&name) || is_trivial_call(&name) {
                    return;
                }

                if !is_meaningful_call(&name, object.as_deref()) {
                    return;
                }

                let is_awaited = node
                    .parent()
                    .map(|p| p.kind() == "await_expression")
                    .unwrap_or(false);

                let node_start = node.start_byte();
                let in_try = try_ranges
                    .iter()
                    .any(|(start, end)| node_start >= *start && node_start < *end);

                let is_io = Call::check_is_io(&name);

                summary.calls.push(Call {
                    name,
                    object,
                    is_awaited,
                    in_try,
                    is_hook: false,
                    is_io,
                    location: Location::new(
                        node.start_position().row + 1,
                        node.start_position().column,
                    ),
                });
            }
        }
    });
}

/// Extract call name and object
fn extract_call_name(func_node: &Node, source: &str) -> (String, Option<String>) {
    match func_node.kind() {
        "identifier" => (get_node_text(func_node, source), None),
        "member_expression" => {
            let property = func_node
                .child_by_field_name("property")
                .map(|p| get_node_text(&p, source))
                .unwrap_or_default();
            let object = func_node
                .child_by_field_name("object")
                .map(|o| simplify_object(&o, source));
            (property, object)
        }
        _ => (get_node_text(func_node, source), None),
    }
}

/// Simplify object reference
fn simplify_object(node: &Node, source: &str) -> String {
    match node.kind() {
        "identifier" => get_node_text(node, source),
        "member_expression" => {
            if let Some(prop) = node.child_by_field_name("property") {
                get_node_text(&prop, source)
            } else {
                get_node_text(node, source)
            }
        }
        "this" => "this".to_string(),
        _ => "_".to_string(),
    }
}

/// Check if call is trivial
fn is_trivial_call(name: &str) -> bool {
    matches!(
        name,
        "log" | "error" | "warn" | "info" | "debug" | "trace" | "toString" | "valueOf"
    )
}

// ============================================================================
// Insertion Generation
// ============================================================================

/// Generate semantic insertions based on file context
fn generate_insertions(summary: &mut SemanticSummary, source: &str) {
    let file_lower = summary.file.to_lowercase();

    // Next.js patterns
    if file_lower.contains("/api/") && file_lower.ends_with("route.ts") {
        if let Some(ref sym) = summary.symbol {
            let method = sym.to_uppercase();
            if matches!(method.as_str(), "GET" | "POST" | "PUT" | "DELETE" | "PATCH") {
                summary
                    .insertions
                    .push(format!("Next.js API route ({})", method));
            }
        }
    }

    if file_lower.ends_with("layout.tsx") || file_lower.ends_with("layout.jsx") {
        if summary.symbol_kind == Some(SymbolKind::Component) {
            summary
                .insertions
                .push("Next.js layout component".to_string());
        }
    }

    if file_lower.ends_with("page.tsx") || file_lower.ends_with("page.jsx") {
        if summary.symbol_kind == Some(SymbolKind::Component) {
            summary.insertions.push("Next.js page component".to_string());
        }
    }

    // Network data fetching
    if source.contains("fetch(") || source.contains("axios") {
        push_unique_insertion(
            &mut summary.insertions,
            "network data fetching".to_string(),
            "network",
        );
    }

    // Config files
    if file_lower.contains("next.config") {
        push_unique_insertion(
            &mut summary.insertions,
            "Next.js configuration".to_string(),
            "Next.js config",
        );
    }
    if file_lower.contains("tailwind.config") {
        push_unique_insertion(
            &mut summary.insertions,
            "Tailwind CSS configuration".to_string(),
            "Tailwind",
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_filename_stem() {
        assert_eq!(extract_filename_stem("/path/to/Header.tsx"), "header");
        assert_eq!(extract_filename_stem("utils.ts"), "utils");
        assert_eq!(extract_filename_stem("index.js"), "index");
    }

    #[test]
    fn test_is_local_import() {
        assert!(is_local_import("./components"));
        assert!(is_local_import("../utils"));
        assert!(!is_local_import("react"));
        assert!(!is_local_import("@/components"));
    }

    #[test]
    fn test_infer_type() {
        assert_eq!(infer_type("\"hello\""), "string");
        assert_eq!(infer_type("42"), "number");
        assert_eq!(infer_type("true"), "boolean");
        assert_eq!(infer_type("[]"), "array");
        assert_eq!(infer_type("{}"), "object");
    }
}
