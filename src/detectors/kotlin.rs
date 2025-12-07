//! Kotlin language detector
//!
//! Extracts semantic information from Kotlin source files using the generic extractor.
//! Kotlin shares many concepts with Java but adds null safety, coroutines, and data classes.

use tree_sitter::Tree;

use crate::detectors::generic::extract_with_grammar;
use crate::detectors::grammar::KOTLIN_GRAMMAR;
use crate::error::Result;
use crate::schema::SemanticSummary;

/// Extract semantic information from a Kotlin source file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    // The generic extractor handles most Kotlin semantics:
    // - Symbols: function_declaration, class_declaration, object_declaration, interface_declaration
    // - Imports: import_header
    // - State changes: property_declaration, variable_declaration, assignment
    // - Control flow: if_expression, when_expression, for_statement, while_statement
    // - Calls: call_expression
    extract_with_grammar(summary, source, tree, &KOTLIN_GRAMMAR)
}
