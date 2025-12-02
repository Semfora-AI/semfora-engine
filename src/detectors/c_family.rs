//! C/C++ language detector

use tree_sitter::{Node, Tree};
use crate::detectors::common::get_node_text;
use crate::error::Result;
use crate::schema::{SemanticSummary, SymbolKind};

pub fn extract(summary: &mut SemanticSummary, _source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();
    let is_header = summary.file.ends_with(".h")
        || summary.file.ends_with(".hpp")
        || summary.file.ends_with(".hxx")
        || summary.file.ends_with(".hh");

    find_primary_symbol(summary, &root, is_header);
    extract_includes(summary, &root);

    Ok(())
}

fn find_primary_symbol(summary: &mut SemanticSummary, root: &Node, is_header: bool) {
    let mut cursor = root.walk();

    for child in root.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(declarator) = child.child_by_field_name("declarator") {
                    if let Some(name) = extract_declarator_name(&declarator, &summary.file) {
                        summary.symbol = Some(name);
                        summary.symbol_kind = Some(SymbolKind::Function);
                        summary.start_line = Some(child.start_position().row + 1);
                        summary.end_line = Some(child.end_position().row + 1);
                        if is_header {
                            summary.public_surface_changed = true;
                        }
                        return;
                    }
                }
            }
            "struct_specifier" | "class_specifier" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    summary.symbol = Some(get_node_text(&name_node, &summary.file));
                    summary.symbol_kind = Some(SymbolKind::Struct);
                    summary.start_line = Some(child.start_position().row + 1);
                    summary.end_line = Some(child.end_position().row + 1);
                    if is_header {
                        summary.public_surface_changed = true;
                    }
                    return;
                }
            }
            "declaration" => {
                let text = get_node_text(&child, &summary.file);
                if text.starts_with("extern") {
                    summary.public_surface_changed = true;
                }
            }
            _ => {}
        }
    }
}

fn extract_declarator_name(node: &Node, file: &str) -> Option<String> {
    match node.kind() {
        "identifier" => Some(get_node_text(node, file)),
        "function_declarator" | "pointer_declarator" => {
            node.child_by_field_name("declarator")
                .and_then(|d| extract_declarator_name(&d, file))
        }
        _ => None,
    }
}

fn extract_includes(summary: &mut SemanticSummary, root: &Node) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "preproc_include" {
            if let Some(path) = child.child_by_field_name("path") {
                let include = get_node_text(&path, &summary.file);
                let clean = include.trim_matches('"').trim_matches('<').trim_matches('>');
                summary.added_dependencies.push(clean.to_string());
            }
        }
    }
}
