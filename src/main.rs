//! MCP-Diff CLI entry point

use std::fs;
use std::process::ExitCode;

use clap::Parser;

use mcp_diff::cli::TokenAnalysisMode;
use mcp_diff::{
    encode_toon, extract, format_analysis_compact, format_analysis_report, Cli, Lang, McpDiffError,
    OutputFormat, TokenAnalyzer,
};

fn main() -> ExitCode {
    match run() {
        Ok(output) => {
            println!("{}", output);
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            e.exit_code()
        }
    }
}

fn run() -> mcp_diff::Result<String> {
    let cli = Cli::parse();

    // 1. Check file exists
    if !cli.file.exists() {
        return Err(McpDiffError::FileNotFound {
            path: cli.file.display().to_string(),
        });
    }

    // 2. Detect language from file extension
    let lang = Lang::from_path(&cli.file)?;

    if cli.verbose {
        eprintln!(
            "Detected language: {} ({})",
            lang.name(),
            lang.family().name()
        );
    }

    // 3. Read source file
    let source = fs::read_to_string(&cli.file)?;

    if cli.verbose {
        eprintln!("Read {} bytes from {}", source.len(), cli.file.display());
    }

    // 4. Parse with tree-sitter
    let mut parser = tree_sitter::Parser::new();
    parser
        .set_language(&lang.tree_sitter_language())
        .map_err(|e| McpDiffError::ParseFailure {
            message: format!("Failed to set language: {:?}", e),
        })?;

    let tree = parser
        .parse(&source, None)
        .ok_or_else(|| McpDiffError::ParseFailure {
            message: "Failed to parse file".to_string(),
        })?;

    if cli.verbose {
        eprintln!("Parsed AST with {} nodes", count_nodes(&tree.root_node()));
    }

    // Optional: Print AST for debugging
    if cli.print_ast {
        eprintln!("\n=== AST ===");
        print_ast(&tree.root_node(), &source, 0);
        eprintln!("=== END AST ===\n");
    }

    // 5. Extract semantic information
    let summary = extract(&cli.file, &source, &tree, lang)?;

    if cli.verbose {
        eprintln!(
            "Extracted: symbol={:?}, deps={}, states={}, control_flow={}",
            summary.symbol,
            summary.added_dependencies.len(),
            summary.state_changes.len(),
            summary.control_flow_changes.len()
        );
    }

    // 6. Encode output in all formats for token analysis
    let toon_output = encode_toon(&summary);
    let json_pretty =
        serde_json::to_string_pretty(&summary).map_err(|e| McpDiffError::ExtractionFailure {
            message: format!("JSON serialization failed: {}", e),
        })?;
    let json_compact =
        serde_json::to_string(&summary).map_err(|e| McpDiffError::ExtractionFailure {
            message: format!("JSON serialization failed: {}", e),
        })?;

    // 7. Run token analysis if requested
    if let Some(mode) = cli.analyze_tokens {
        let analyzer = TokenAnalyzer::new();
        let analysis = analyzer.analyze(&source, &json_pretty, &json_compact, &toon_output);

        let report = match mode {
            TokenAnalysisMode::Full => format_analysis_report(&analysis, cli.compare_compact),
            TokenAnalysisMode::Compact => format_analysis_compact(&analysis, cli.compare_compact),
        };
        eprintln!("{}", report);
    }

    // 8. Return output in requested format
    let output = match cli.format {
        OutputFormat::Toon => toon_output,
        OutputFormat::Json => json_pretty,
    };

    Ok(output)
}

/// Count total nodes in the AST
fn count_nodes(node: &tree_sitter::Node) -> usize {
    let mut count = 1;
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        count += count_nodes(&child);
    }
    count
}

/// Print AST for debugging
fn print_ast(node: &tree_sitter::Node, source: &str, depth: usize) {
    let indent = "  ".repeat(depth);
    let text = node
        .utf8_text(source.as_bytes())
        .unwrap_or("<invalid utf8>");
    let text_preview: String = text.chars().take(50).collect();
    let text_preview = text_preview.replace('\n', "\\n");

    eprintln!(
        "{}{}:{} [{}-{}] \"{}\"{}",
        indent,
        node.kind(),
        if node.is_named() { "" } else { " (anonymous)" },
        node.start_position().row,
        node.end_position().row,
        text_preview,
        if text.len() > 50 { "..." } else { "" }
    );

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        print_ast(&child, source, depth + 1);
    }
}
