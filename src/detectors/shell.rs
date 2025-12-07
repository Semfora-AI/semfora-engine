//! Shell/Bash language detector
//!
//! Extracts semantic information from shell scripts using the generic extractor.
//! Supports bash, sh, zsh, and fish syntax (parsed with bash grammar).

use tree_sitter::Tree;

use crate::detectors::generic::extract_with_grammar;
use crate::detectors::grammar::BASH_GRAMMAR;
use crate::error::Result;
use crate::schema::SemanticSummary;

/// Extract semantic information from a shell script
pub fn extract(summary: &mut SemanticSummary, source: &str, tree: &Tree) -> Result<()> {
    // The generic extractor handles shell semantics:
    // - Symbols: function_definition
    // - State changes: variable_assignment
    // - Control flow: if_statement, case_statement, for_statement, while_statement
    // - Calls: command (function and program invocations)
    extract_with_grammar(summary, source, tree, &BASH_GRAMMAR)
}
