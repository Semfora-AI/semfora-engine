//! Markup language detector (HTML, CSS, Markdown)

use tree_sitter::Tree;
use crate::error::Result;
use crate::schema::SemanticSummary;

pub fn extract(summary: &mut SemanticSummary, _source: &str, _tree: &Tree) -> Result<()> {
    // Markup files have simpler extraction - mainly structure
    // For now, just mark as complete with the file info
    summary.extraction_complete = true;
    Ok(())
}
