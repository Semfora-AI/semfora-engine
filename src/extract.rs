//! Semantic extraction orchestration
//!
//! This module coordinates the extraction of semantic information from parsed
//! source files using language-specific detectors.

use std::path::Path;
use tree_sitter::Tree;

use crate::error::Result;
use crate::lang::Lang;
use crate::risk::calculate_risk;
use crate::schema::SemanticSummary;

/// Extract semantic information from a parsed source file
///
/// This is the main entry point for semantic extraction. It delegates to
/// language-specific extractors based on the detected language.
pub fn extract(file_path: &Path, source: &str, tree: &Tree, lang: Lang) -> Result<SemanticSummary> {
    let mut summary = SemanticSummary {
        file: file_path.display().to_string(),
        language: lang.name().to_string(),
        ..Default::default()
    };

    // Dispatch to language family extractor
    match lang.family() {
        crate::lang::LangFamily::JavaScript => {
            extract_javascript_family(&mut summary, source, tree, lang)?;
        }
        crate::lang::LangFamily::Rust => {
            extract_rust(&mut summary, source, tree)?;
        }
        crate::lang::LangFamily::Python => {
            extract_python(&mut summary, source, tree)?;
        }
        crate::lang::LangFamily::Go => {
            extract_go(&mut summary, source, tree)?;
        }
        crate::lang::LangFamily::Java => {
            extract_java(&mut summary, source, tree)?;
        }
        crate::lang::LangFamily::CFamily => {
            extract_c_family(&mut summary, source, tree, lang)?;
        }
        crate::lang::LangFamily::Markup => {
            extract_markup(&mut summary, source, tree, lang)?;
        }
        crate::lang::LangFamily::Config => {
            extract_config(&mut summary, source, tree, lang)?;
        }
    }

    // Calculate risk score
    summary.behavioral_risk = calculate_risk(&summary);

    // Mark extraction as complete if we got a symbol
    summary.extraction_complete = summary.symbol.is_some();

    // Add raw fallback if extraction was incomplete
    if !summary.extraction_complete {
        // Truncate source for fallback if too long
        let max_fallback_len = 1000;
        if source.len() > max_fallback_len {
            summary.raw_fallback = Some(format!("{}...", &source[..max_fallback_len]));
        } else {
            summary.raw_fallback = Some(source.to_string());
        }
    }

    Ok(summary)
}

/// Extract from JavaScript/TypeScript/JSX/TSX files
fn extract_javascript_family(
    summary: &mut SemanticSummary,
    source: &str,
    tree: &Tree,
    lang: Lang,
) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol (function, class, component)
    find_primary_symbol_js(summary, &root, source);

    // Extract imports
    extract_imports_js(summary, &root, source);

    // Extract state hooks (useState, useReducer)
    extract_state_hooks(summary, &root, source);

    // Extract JSX elements for insertion rules
    if lang.supports_jsx() {
        extract_jsx_insertions(summary, &root, source);
    }

    // Extract control flow
    extract_control_flow_js(summary, &root, source);

    Ok(())
}

/// Extract from Rust files
fn extract_rust(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol
    find_primary_symbol_rust(summary, &root, source);

    // Extract use statements
    extract_imports_rust(summary, &root, source);

    // Extract let bindings
    extract_state_rust(summary, &root, source);

    // Extract control flow
    extract_control_flow_rust(summary, &root, source);

    Ok(())
}

/// Extract from Python files
fn extract_python(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol
    find_primary_symbol_python(summary, &root, source);

    // Extract imports
    extract_imports_python(summary, &root, source);

    // Extract variable assignments
    extract_state_python(summary, &root, source);

    // Extract control flow
    extract_control_flow_python(summary, &root, source);

    Ok(())
}

/// Extract from Go files
fn extract_go(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol
    find_primary_symbol_go(summary, &root, source);

    // Extract imports
    extract_imports_go(summary, &root, source);

    Ok(())
}

/// Extract from Java files
fn extract_java(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol (class)
    find_primary_symbol_java(summary, &root, source);

    // Extract imports
    extract_imports_java(summary, &root, source);

    Ok(())
}

/// Extract from C/C++ files
fn extract_c_family(
    summary: &mut SemanticSummary,
    source: &str,
    tree: &Tree,
    _lang: Lang,
) -> Result<()> {
    let root = tree.root_node();

    // Find primary symbol
    find_primary_symbol_c(summary, &root, source);

    // Extract includes
    extract_includes_c(summary, &root, source);

    Ok(())
}

/// Extract from markup files (HTML, CSS, Markdown)
fn extract_markup(
    summary: &mut SemanticSummary,
    _source: &str,
    _tree: &Tree,
    _lang: Lang,
) -> Result<()> {
    // Markup files have simpler extraction - mainly structure
    // For now, just mark as complete with the file info
    summary.extraction_complete = true;
    Ok(())
}

/// Extract from config files (JSON, YAML, TOML)
fn extract_config(
    summary: &mut SemanticSummary,
    _source: &str,
    _tree: &Tree,
    _lang: Lang,
) -> Result<()> {
    // Config files have simpler extraction - mainly structure
    // For now, just mark as complete with the file info
    summary.extraction_complete = true;
    Ok(())
}

// ============================================================================
// JavaScript/TypeScript extraction helpers
// ============================================================================

fn find_primary_symbol_js(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    source: &str,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "export_statement" => {
                // Look for default export or named export
                if let Some(decl) = child.child_by_field_name("declaration") {
                    extract_symbol_from_declaration_js(summary, &decl, source);
                } else {
                    // Check for direct function/class inside export
                    let mut inner_cursor = child.walk();
                    for inner in child.children(&mut inner_cursor) {
                        if inner.kind() == "function_declaration"
                            || inner.kind() == "class_declaration"
                        {
                            extract_symbol_from_declaration_js(summary, &inner, source);
                            break;
                        }
                    }
                }
                if summary.symbol.is_some() {
                    summary.public_surface_changed = true;
                    return;
                }
            }
            "function_declaration" | "class_declaration" | "lexical_declaration" => {
                extract_symbol_from_declaration_js(summary, &child, source);
                if summary.symbol.is_some() {
                    return;
                }
            }
            _ => {}
        }
    }
}

fn extract_symbol_from_declaration_js(
    summary: &mut SemanticSummary,
    node: &tree_sitter::Node,
    source: &str,
) {
    match node.kind() {
        "function_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                summary.symbol = Some(get_node_text(&name_node, source));
                summary.symbol_kind = Some(crate::schema::SymbolKind::Function);

                // Check if it returns JSX (making it a component)
                if returns_jsx(node, source) {
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Component);
                    summary.return_type = Some("JSX.Element".to_string());
                }
            }
        }
        "class_declaration" => {
            if let Some(name_node) = node.child_by_field_name("name") {
                summary.symbol = Some(get_node_text(&name_node, source));
                summary.symbol_kind = Some(crate::schema::SymbolKind::Class);
            }
        }
        "lexical_declaration" => {
            // Look for arrow function assigned to const
            let mut cursor = node.walk();
            for child in node.children(&mut cursor) {
                if child.kind() == "variable_declarator" {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        if let Some(value_node) = child.child_by_field_name("value") {
                            if value_node.kind() == "arrow_function" {
                                summary.symbol = Some(get_node_text(&name_node, source));
                                summary.symbol_kind = Some(crate::schema::SymbolKind::Function);

                                if returns_jsx(&value_node, source) {
                                    summary.symbol_kind =
                                        Some(crate::schema::SymbolKind::Component);
                                    summary.return_type = Some("JSX.Element".to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn returns_jsx(node: &tree_sitter::Node, source: &str) -> bool {
    let text = get_node_text(node, source);
    text.contains("jsx_element")
        || text.contains("<")
        || contains_node_kind(node, "jsx_element")
        || contains_node_kind(node, "jsx_self_closing_element")
}

fn contains_node_kind(node: &tree_sitter::Node, kind: &str) -> bool {
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

fn extract_imports_js(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_statement" {
            // Get imported names from import clause
            if let Some(clause) = child.child_by_field_name("source") {
                let module = get_node_text(&clause, source);
                let module = module.trim_matches('"').trim_matches('\'');

                // Get specific imports
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "import_clause" {
                        extract_import_names(summary, &inner, source, module);
                    }
                }
            }
        }
    }
}

fn extract_import_names(
    summary: &mut SemanticSummary,
    clause: &tree_sitter::Node,
    source: &str,
    _module: &str,
) {
    let mut cursor = clause.walk();

    for child in clause.children(&mut cursor) {
        match child.kind() {
            "identifier" => {
                // Default import
                summary
                    .added_dependencies
                    .push(get_node_text(&child, source));
            }
            "named_imports" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "import_specifier" {
                        if let Some(name) = inner.child_by_field_name("name") {
                            summary
                                .added_dependencies
                                .push(get_node_text(&name, source));
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_state_hooks(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    visit_all(root, |node| {
        if node.kind() == "call_expression" {
            if let Some(func) = node.child_by_field_name("function") {
                let func_name = get_node_text(&func, source);
                if func_name == "useState" || func_name == "useReducer" {
                    // Look for the variable declarator parent to get the state name
                    if let Some(parent) = node.parent() {
                        if parent.kind() == "variable_declarator" {
                            if let Some(name_node) = parent.child_by_field_name("name") {
                                if name_node.kind() == "array_pattern" {
                                    // Get first element of destructuring
                                    let mut cursor = name_node.walk();
                                    for child in name_node.children(&mut cursor) {
                                        if child.kind() == "identifier" {
                                            let state_name = get_node_text(&child, source);

                                            // Get initializer
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

                                            summary.state_changes.push(crate::schema::StateChange {
                                                name: state_name,
                                                state_type: "boolean".to_string(), // Simplified
                                                initializer: init,
                                            });

                                            // Add insertion for state hook
                                            summary.insertions.push(format!(
                                                "local {} state via {}",
                                                get_node_text(&child, source),
                                                func_name
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

fn extract_jsx_insertions(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut jsx_tags: Vec<String> = Vec::new();

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
    });

    // Apply insertion rules
    if jsx_tags.iter().any(|t| t == "header") {
        if jsx_tags.iter().any(|t| t == "nav") {
            summary
                .insertions
                .push("header container with nav".to_string());
        } else {
            summary.insertions.push("header container".to_string());
        }
    }

    let link_count = jsx_tags.iter().filter(|t| *t == "Link" || *t == "a").count();
    if link_count >= 3 {
        summary
            .insertions
            .push(format!("{} route links", link_count));
    }

    if jsx_tags.iter().any(|t| t == "button")
        && jsx_tags.iter().any(|t| t == "div" || t == "menu")
    {
        summary.insertions.push("dropdown menu".to_string());
    }
}

fn extract_control_flow_js(summary: &mut SemanticSummary, root: &tree_sitter::Node, _source: &str) {
    visit_all(root, |node| {
        let kind = match node.kind() {
            "if_statement" => Some(crate::schema::ControlFlowKind::If),
            "for_statement" | "for_in_statement" => Some(crate::schema::ControlFlowKind::For),
            "while_statement" => Some(crate::schema::ControlFlowKind::While),
            "switch_statement" => Some(crate::schema::ControlFlowKind::Switch),
            "try_statement" => Some(crate::schema::ControlFlowKind::Try),
            _ => None,
        };

        if let Some(k) = kind {
            summary
                .control_flow_changes
                .push(crate::schema::ControlFlowChange {
                    kind: k,
                    location: crate::schema::Location::new(
                        node.start_position().row + 1,
                        node.start_position().column,
                    ),
                });
        }
    });
}

// ============================================================================
// Rust extraction helpers
// ============================================================================

fn find_primary_symbol_rust(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    source: &str,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Function);

                    // Check for pub visibility
                    let mut vis_cursor = child.walk();
                    for vis_child in child.children(&mut vis_cursor) {
                        if vis_child.kind() == "visibility_modifier" {
                            summary.public_surface_changed = true;
                            break;
                        }
                    }
                    return;
                }
            }
            "struct_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Struct);
                    return;
                }
            }
            "impl_item" => {
                if let Some(type_node) = child.child_by_field_name("type") {
                    summary.symbol = Some(get_node_text(&type_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Method);
                    return;
                }
            }
            "trait_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Trait);
                    return;
                }
            }
            "enum_item" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Enum);
                    return;
                }
            }
            _ => {}
        }
    }
}

fn extract_imports_rust(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "use_declaration" {
            // Get the full use path
            if let Some(arg) = child.child_by_field_name("argument") {
                let use_text = get_node_text(&arg, source);
                // Extract the last segment as the imported name
                if let Some(last) = use_text.split("::").last() {
                    let name = last.trim_matches('{').trim_matches('}').trim();
                    if !name.is_empty() && name != "*" {
                        summary.added_dependencies.push(name.to_string());
                    }
                }
            }
        }
    }
}

fn extract_state_rust(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    visit_all(root, |node| {
        if node.kind() == "let_declaration" {
            if let Some(pattern) = node.child_by_field_name("pattern") {
                let name = get_node_text(&pattern, source);
                let type_str = node
                    .child_by_field_name("type")
                    .map(|t| get_node_text(&t, source))
                    .unwrap_or_else(|| "_".to_string());
                let init = node
                    .child_by_field_name("value")
                    .map(|v| get_node_text(&v, source))
                    .unwrap_or_else(|| "_".to_string());

                summary.state_changes.push(crate::schema::StateChange {
                    name,
                    state_type: type_str,
                    initializer: init,
                });
            }
        }
    });
}

fn extract_control_flow_rust(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    _source: &str,
) {
    visit_all(root, |node| {
        let kind = match node.kind() {
            "if_expression" => Some(crate::schema::ControlFlowKind::If),
            "for_expression" => Some(crate::schema::ControlFlowKind::For),
            "while_expression" => Some(crate::schema::ControlFlowKind::While),
            "match_expression" => Some(crate::schema::ControlFlowKind::Match),
            "loop_expression" => Some(crate::schema::ControlFlowKind::Loop),
            _ => None,
        };

        if let Some(k) = kind {
            summary
                .control_flow_changes
                .push(crate::schema::ControlFlowChange {
                    kind: k,
                    location: crate::schema::Location::new(
                        node.start_position().row + 1,
                        node.start_position().column,
                    ),
                });
        }
    });
}

// ============================================================================
// Python extraction helpers
// ============================================================================

fn find_primary_symbol_python(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    source: &str,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Function);
                    return;
                }
            }
            "class_definition" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, source));
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Class);
                    return;
                }
            }
            "decorated_definition" => {
                // Look inside decorated definition
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "function_definition" || inner.kind() == "class_definition" {
                        find_primary_symbol_python(summary, &inner, source);
                        if summary.symbol.is_some() {
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_imports_python(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "import_statement" => {
                if let Some(name) = child.child_by_field_name("name") {
                    summary
                        .added_dependencies
                        .push(get_node_text(&name, source));
                }
            }
            "import_from_statement" => {
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "dotted_name" || inner.kind() == "aliased_import" {
                        summary
                            .added_dependencies
                            .push(get_node_text(&inner, source));
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_state_python(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "expression_statement" {
            let mut inner_cursor = child.walk();
            for inner in child.children(&mut inner_cursor) {
                if inner.kind() == "assignment" {
                    if let Some(left) = inner.child_by_field_name("left") {
                        if let Some(right) = inner.child_by_field_name("right") {
                            summary.state_changes.push(crate::schema::StateChange {
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

fn extract_control_flow_python(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    _source: &str,
) {
    visit_all(root, |node| {
        let kind = match node.kind() {
            "if_statement" => Some(crate::schema::ControlFlowKind::If),
            "for_statement" => Some(crate::schema::ControlFlowKind::For),
            "while_statement" => Some(crate::schema::ControlFlowKind::While),
            "try_statement" => Some(crate::schema::ControlFlowKind::Try),
            "match_statement" => Some(crate::schema::ControlFlowKind::Match),
            _ => None,
        };

        if let Some(k) = kind {
            summary
                .control_flow_changes
                .push(crate::schema::ControlFlowChange {
                    kind: k,
                    location: crate::schema::Location::new(
                        node.start_position().row + 1,
                        node.start_position().column,
                    ),
                });
        }
    });
}

// ============================================================================
// Go extraction helpers
// ============================================================================

fn find_primary_symbol_go(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    source: &str,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = get_node_text(&name_node, source);
                    summary.symbol = Some(name.clone());
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Function);

                    // Check if exported (starts with uppercase)
                    if name.chars().next().map(|c| c.is_uppercase()).unwrap_or(false) {
                        summary.public_surface_changed = true;
                    }
                    return;
                }
            }
            "type_declaration" => {
                // Look for struct or interface
                let mut inner_cursor = child.walk();
                for inner in child.children(&mut inner_cursor) {
                    if inner.kind() == "type_spec" {
                        if let Some(name_node) = inner.child_by_field_name("name") {
                            summary.symbol = Some(get_node_text(&name_node, source));
                            summary.symbol_kind = Some(crate::schema::SymbolKind::Struct);
                            return;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_imports_go(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
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

// ============================================================================
// Java extraction helpers
// ============================================================================

fn find_primary_symbol_java(
    summary: &mut SemanticSummary,
    root: &tree_sitter::Node,
    source: &str,
) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "class_declaration" {
            if let Some(name_node) = child.child_by_field_name("name") {
                summary.symbol = Some(get_node_text(&name_node, source));
                summary.symbol_kind = Some(crate::schema::SymbolKind::Class);

                // Check for public modifier
                let mut mod_cursor = child.walk();
                for mod_child in child.children(&mut mod_cursor) {
                    if mod_child.kind() == "modifiers" {
                        let mods = get_node_text(&mod_child, source);
                        if mods.contains("public") {
                            summary.public_surface_changed = true;
                        }
                    }
                }
                return;
            }
        }
    }
}

fn extract_imports_java(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "import_declaration" {
            let import_text = get_node_text(&child, source);
            // Extract the class name from the import
            let clean = import_text.trim_start_matches("import ");
            let clean = clean.trim_end_matches(';');
            if let Some(last) = clean.split('.').last() {
                if last != "*" {
                    summary.added_dependencies.push(last.to_string());
                }
            }
        }
    }
}

// ============================================================================
// C/C++ extraction helpers
// ============================================================================

fn find_primary_symbol_c(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "function_definition" {
            if let Some(declarator) = child.child_by_field_name("declarator") {
                // Navigate to find the function name
                let name = extract_declarator_name(&declarator, source);
                if let Some(n) = name {
                    summary.symbol = Some(n);
                    summary.symbol_kind = Some(crate::schema::SymbolKind::Function);
                    return;
                }
            }
        }
    }
}

fn extract_declarator_name(node: &tree_sitter::Node, source: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(get_node_text(node, source)),
        "function_declarator" => {
            if let Some(declarator) = node.child_by_field_name("declarator") {
                extract_declarator_name(&declarator, source)
            } else {
                None
            }
        }
        "pointer_declarator" => {
            if let Some(declarator) = node.child_by_field_name("declarator") {
                extract_declarator_name(&declarator, source)
            } else {
                None
            }
        }
        _ => None,
    }
}

fn extract_includes_c(summary: &mut SemanticSummary, root: &tree_sitter::Node, source: &str) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        if child.kind() == "preproc_include" {
            if let Some(path) = child.child_by_field_name("path") {
                let include = get_node_text(&path, source);
                let clean = include.trim_matches('"').trim_matches('<').trim_matches('>');
                summary.added_dependencies.push(clean.to_string());
            }
        }
    }
}

// ============================================================================
// Utility functions
// ============================================================================

/// Get text content of a node
fn get_node_text(node: &tree_sitter::Node, source: &str) -> String {
    node.utf8_text(source.as_bytes())
        .unwrap_or("")
        .to_string()
}

/// Visit all nodes in a tree
fn visit_all<F>(node: &tree_sitter::Node, mut visitor: F)
where
    F: FnMut(&tree_sitter::Node),
{
    visit_all_recursive(node, &mut visitor);
}

fn visit_all_recursive<F>(node: &tree_sitter::Node, visitor: &mut F)
where
    F: FnMut(&tree_sitter::Node),
{
    visitor(node);
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        visit_all_recursive(&child, visitor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn parse_source(source: &str, lang: Lang) -> Tree {
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&lang.tree_sitter_language()).unwrap();
        parser.parse(source, None).unwrap()
    }

    #[test]
    fn test_extract_tsx_component() {
        let source = r#"
import { useState } from "react";
import { Link } from "react-router-dom";

export default function AppLayout() {
    const [open, setOpen] = useState(false);
    return <div><header><nav><Link to="/a" /></nav></header></div>;
}
"#;

        let tree = parse_source(source, Lang::Tsx);
        let path = PathBuf::from("test.tsx");
        let summary = extract(&path, source, &tree, Lang::Tsx).unwrap();

        assert_eq!(summary.symbol, Some("AppLayout".to_string()));
        assert!(summary.public_surface_changed);
        assert!(!summary.added_dependencies.is_empty());
    }

    #[test]
    fn test_extract_rust_function() {
        let source = r#"
use std::io::Result;

pub fn main() -> Result<()> {
    let x = 42;
    if x > 0 {
        println!("positive");
    }
    Ok(())
}
"#;

        let tree = parse_source(source, Lang::Rust);
        let path = PathBuf::from("test.rs");
        let summary = extract(&path, source, &tree, Lang::Rust).unwrap();

        assert_eq!(summary.symbol, Some("main".to_string()));
        assert!(summary.public_surface_changed);
    }

    #[test]
    fn test_extract_python_function() {
        let source = r#"
import os
from typing import List

def process_files(paths: List[str]) -> None:
    for path in paths:
        if os.path.exists(path):
            print(path)
"#;

        let tree = parse_source(source, Lang::Python);
        let path = PathBuf::from("test.py");
        let summary = extract(&path, source, &tree, Lang::Python).unwrap();

        assert_eq!(summary.symbol, Some("process_files".to_string()));
        assert!(!summary.added_dependencies.is_empty());
    }
}
