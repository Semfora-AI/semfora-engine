//! Gradle build file detector
//!
//! Extracts semantic information from Gradle build files using the generic extractor.
//! Gradle files use Groovy syntax (build.gradle) or Kotlin DSL (build.gradle.kts).
//! This detector handles the Groovy-based .gradle files.

use tree_sitter::Tree;

use crate::detectors::generic::extract_with_grammar;
use crate::detectors::grammar::GRADLE_GRAMMAR;
use crate::error::Result;
use crate::schema::SemanticSummary;

/// Extract semantic information from a Gradle build file
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    // The generic extractor handles Gradle/Groovy semantics:
    // - Symbols: method_declaration, closure
    // - Imports: import_declaration
    // - State changes: variable_declaration, assignment
    // - Control flow: if_statement, for_statement, while_statement
    // - Calls: method_call_expression
    extract_with_grammar(summary, source, tree, &GRADLE_GRAMMAR)
}
