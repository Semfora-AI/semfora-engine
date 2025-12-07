//! Java language detector
//!
//! Extracts semantic information from Java source files using the generic extractor.
//! Java's class/interface/enum declarations are first-class AST nodes, so the generic
//! extractor handles them well.

use tree_sitter::Tree;

use crate::detectors::generic::extract_with_grammar;
use crate::detectors::grammar::JAVA_GRAMMAR;
use crate::error::Result;
use crate::schema::SemanticSummary;

/// Extract semantic information from a Java source file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    // The generic extractor handles everything for Java:
    // - Symbols: class_declaration, interface_declaration, enum_declaration, method_declaration
    // - Imports: import_declaration
    // - State changes: local_variable_declaration, field_declaration, assignment_expression
    // - Control flow: if, for, enhanced_for, while, do, switch, try
    // - Calls: method_invocation
    // - Risk calculation
    extract_with_grammar(summary, source, tree, &JAVA_GRAMMAR)
}
