//! CLI argument definitions using clap

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

use crate::tokens::{format_analysis_compact, format_analysis_report, TokenAnalyzer};

/// Semantic code analyzer with TOON output
#[derive(Parser, Debug)]
#[command(name = "mcp-diff")]
#[command(about = "Deterministic semantic code analyzer that outputs TOON-formatted summaries")]
#[command(version)]
#[command(author)]
pub struct Cli {
    /// Path to file to analyze
    #[arg(value_name = "FILE")]
    pub file: PathBuf,

    /// Output format
    #[arg(short, long, default_value = "toon", value_enum)]
    pub format: OutputFormat,

    /// Show verbose output including AST info
    #[arg(short, long)]
    pub verbose: bool,

    /// Print the parsed AST (for debugging)
    #[arg(long)]
    pub print_ast: bool,

    /// Analyze AI token counts (pre/post TOON conversion)
    #[arg(long, value_enum)]
    pub analyze_tokens: Option<TokenAnalysisMode>,

    /// Include compact JSON in token analysis comparison
    #[arg(long, requires = "analyze_tokens")]
    pub compare_compact: bool,
}

/// Token analysis output mode
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum TokenAnalysisMode {
    /// Full detailed report with breakdown
    Full,
    /// Compact single-line summary
    Compact,
}

/// Output format options
#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum OutputFormat {
    /// TOON (Token-Oriented Object Notation) - default, token-efficient format
    #[default]
    Toon,
    /// JSON - standard JSON output
    Json,
}

impl Cli {
    /// Parse CLI arguments from command line
    pub fn parse_args() -> Self {
        Self::parse()
    }

    /// Run token analysis on the given source, JSON (pretty & compact), and TOON outputs
    pub fn run_token_analysis(
        &self,
        source: &str,
        json_pretty: &str,
        json_compact: &str,
        toon: &str,
    ) -> Option<String> {
        self.analyze_tokens.map(|mode| {
            let analyzer = TokenAnalyzer::new();
            let analysis = analyzer.analyze(source, json_pretty, json_compact, toon);

            match mode {
                TokenAnalysisMode::Full => format_analysis_report(&analysis, self.compare_compact),
                TokenAnalysisMode::Compact => {
                    format_analysis_compact(&analysis, self.compare_compact)
                }
            }
        })
    }
}
