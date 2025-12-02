//! Java language detector

use tree_sitter::{Node, Tree};
use crate::detectors::common::get_node_text;
use crate::error::Result;
use crate::schema::{SemanticSummary, SymbolKind};

pub fn extract(summary: &mut SemanticSummary, _source: &str, tree: &Tree) -> Result<()> {
    let root = tree.root_node();
    let filename_stem = extract_filename_stem(&summary.file);

    find_primary_symbol(summary, &root, &filename_stem);
    extract_imports(summary, &root);

    Ok(())
}

fn extract_filename_stem(file_path: &str) -> String {
    std::path::Path::new(file_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string()
}

fn find_primary_symbol(summary: &mut SemanticSummary, root: &Node, filename_stem: &str) {
    let mut cursor = root.walk();
    let mut best_score = -1;

    for child in root.children(&mut cursor) {
        match child.kind() {
            "class_declaration" | "interface_declaration" | "enum_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = get_node_text(&name_node, &summary.file);
                    let is_public = has_public_modifier(&child);
                    let kind = match child.kind() {
                        "interface_declaration" => SymbolKind::Trait,
                        "enum_declaration" => SymbolKind::Enum,
                        _ => SymbolKind::Class,
                    };

                    let mut score = if is_public { 50 } else { 0 };
                    score += match kind {
                        SymbolKind::Class => 30,
                        SymbolKind::Trait => 25,
                        SymbolKind::Enum => 20,
                        _ => 10,
                    };
                    if name.to_lowercase() == filename_stem.to_lowercase() {
                        score += 40;
                    }

                    if score > best_score {
                        best_score = score;
                        summary.symbol = Some(name);
                        summary.symbol_kind = Some(kind);
                        summary.start_line = Some(child.start_position().row + 1);
                        summary.end_line = Some(child.end_position().row + 1);
                        summary.public_surface_changed = is_public;
                    }
                }
            }
            _ => {}
        }
    }
}

fn has_public_modifier(node: &Node) -> bool {
    if let Some(modifiers) = node.child_by_field_name("modifiers") {
        let mut cursor = modifiers.walk();
        for child in modifiers.children(&mut cursor) {
            if child.kind() == "public" {
                return true;
            }
        }
    }
    false
}

fn extract_imports(summary: &mut SemanticSummary, root: &Node) {
    let mut cursor = root.walk();
    for child in root.children(&mut cursor) {
        if child.kind() == "import_declaration" {
            if let Some(scope) = child.child_by_field_name("scope") {
                let import_text = get_node_text(&scope, &summary.file);
                if let Some(last) = import_text.split('.').last() {
                    if last != "*" {
                        summary.added_dependencies.push(last.to_string());
                    }
                }
            }
        }
    }
}
