//! AI Token Analyzer
//!
//! Provides estimation of AI model token counts for analyzing the efficiency
//! of TOON (Token-Oriented Object Notation) encoding compared to other formats.
//!
//! This module uses a simplified BPE-style estimation that approximates
//! token counts for models like GPT-4, Claude, etc.

use std::collections::HashMap;

/// Token analysis result comparing different formats
#[derive(Debug, Clone, Default)]
pub struct TokenAnalysis {
    /// Original source code token count
    pub source_tokens: usize,
    /// JSON format (pretty-printed) token count
    pub json_tokens: usize,
    /// Compact JSON format token count
    pub json_compact_tokens: usize,
    /// TOON format token count
    pub toon_tokens: usize,
    /// Token savings (JSON pretty - TOON)
    pub token_savings: i64,
    /// Token savings vs compact JSON (JSON compact - TOON)
    pub token_savings_vs_compact: i64,
    /// Percentage reduction from pretty JSON to TOON
    pub reduction_percent: f64,
    /// Percentage reduction from compact JSON to TOON
    pub reduction_percent_vs_compact: f64,
    /// Detailed breakdown by content type
    pub breakdown: TokenBreakdown,
}

/// Detailed breakdown of token counts by content type
#[derive(Debug, Clone, Default)]
pub struct TokenBreakdown {
    /// Tokens from field names/keys
    pub field_names: usize,
    /// Tokens from values
    pub values: usize,
    /// Tokens from structural characters (braces, brackets, quotes)
    pub structural: usize,
    /// Tokens from whitespace
    pub whitespace: usize,
}

/// AI Token Analyzer using BPE-style estimation
pub struct TokenAnalyzer {
    /// Common programming tokens that are typically single tokens in BPE
    common_tokens: HashMap<&'static str, usize>,
}

impl Default for TokenAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenAnalyzer {
    /// Create a new token analyzer with default token mappings
    pub fn new() -> Self {
        let mut common_tokens = HashMap::new();

        // Common programming keywords (usually 1 token each)
        for token in &[
            "function",
            "return",
            "const",
            "let",
            "var",
            "if",
            "else",
            "for",
            "while",
            "switch",
            "case",
            "break",
            "continue",
            "import",
            "export",
            "from",
            "class",
            "interface",
            "type",
            "enum",
            "struct",
            "impl",
            "fn",
            "pub",
            "mod",
            "use",
            "async",
            "await",
            "try",
            "catch",
            "throw",
            "new",
            "this",
            "self",
            "true",
            "false",
            "null",
            "undefined",
            "None",
            "True",
            "False",
            "def",
            "lambda",
        ] {
            common_tokens.insert(*token, 1);
        }

        // Common symbols (usually 1 token each)
        for token in &[
            "{", "}", "[", "]", "(", ")", ":", ",", ";", ".", "=", "=>", "->", "==", "!=", "<=",
            ">=", "<", ">", "+", "-", "*", "/", "%", "&&", "||", "!", "&", "|", "^", "~", "<<",
            ">>", "++", "--", "+=", "-=", "*=", "/=",
        ] {
            common_tokens.insert(*token, 1);
        }

        // Common field names in semantic analysis (usually 1-2 tokens)
        for token in &[
            "file",
            "language",
            "symbol",
            "type",
            "name",
            "value",
            "default",
            "required",
            "public",
            "private",
            "static",
            "arguments",
            "props",
            "insertions",
            "dependencies",
            "state",
            "control",
            "flow",
            "risk",
            "behavioral",
            "surface",
            "changed",
            "raw",
            "fallback",
        ] {
            common_tokens.insert(*token, 1);
        }

        Self { common_tokens }
    }

    /// Estimate token count for a string using BPE-style rules
    pub fn count_tokens(&self, text: &str) -> usize {
        let mut total = 0;
        let mut chars = text.chars().peekable();
        let mut current_word = String::new();

        while let Some(c) = chars.next() {
            match c {
                // Whitespace handling
                ' ' | '\t' => {
                    if !current_word.is_empty() {
                        total += self.estimate_word_tokens(&current_word);
                        current_word.clear();
                    }
                    // Consecutive whitespace often merges into 1 token
                    let mut ws_count = 1;
                    while chars
                        .peek()
                        .map(|&c| c == ' ' || c == '\t')
                        .unwrap_or(false)
                    {
                        chars.next();
                        ws_count += 1;
                    }
                    // Approximate: 4-5 spaces = 1 token
                    total += (ws_count + 4) / 5;
                }
                '\n' | '\r' => {
                    if !current_word.is_empty() {
                        total += self.estimate_word_tokens(&current_word);
                        current_word.clear();
                    }
                    total += 1; // Newlines are typically 1 token
                }
                // Structural characters
                '{' | '}' | '[' | ']' | '(' | ')' | ':' | ',' | ';' | '"' | '\'' => {
                    if !current_word.is_empty() {
                        total += self.estimate_word_tokens(&current_word);
                        current_word.clear();
                    }
                    total += 1;
                }
                // Build up words
                _ => {
                    current_word.push(c);
                }
            }
        }

        // Handle remaining word
        if !current_word.is_empty() {
            total += self.estimate_word_tokens(&current_word);
        }

        total.max(1) // Minimum 1 token
    }

    /// Estimate tokens for a single word
    fn estimate_word_tokens(&self, word: &str) -> usize {
        // Check if it's a known common token
        if let Some(&count) = self.common_tokens.get(word) {
            return count;
        }

        // Check lowercase version
        let lower = word.to_lowercase();
        if let Some(&count) = self.common_tokens.get(lower.as_str()) {
            return count;
        }

        // BPE estimation rules:
        // - Short words (1-4 chars): usually 1 token
        // - Medium words (5-10 chars): usually 1-2 tokens
        // - Long words: approximately 1 token per 4 characters
        // - Numbers: 1 token per 3-4 digits
        // - camelCase/snake_case: split and count

        let len = word.len();

        if word.chars().all(|c| c.is_ascii_digit()) {
            // Pure numbers: ~3-4 digits per token
            return (len + 3) / 4;
        }

        // Check for compound identifiers (camelCase or snake_case)
        let parts = split_identifier(word);
        if parts.len() > 1 {
            return parts.iter().map(|p| self.estimate_word_tokens(p)).sum();
        }

        // Simple word estimation
        match len {
            0 => 0,
            1..=4 => 1,
            5..=10 => 2,
            _ => (len + 3) / 4,
        }
    }

    /// Analyze token counts for source, JSON (pretty & compact), and TOON formats
    pub fn analyze(
        &self,
        source: &str,
        json_pretty: &str,
        json_compact: &str,
        toon: &str,
    ) -> TokenAnalysis {
        let source_tokens = self.count_tokens(source);
        let json_tokens = self.count_tokens(json_pretty);
        let json_compact_tokens = self.count_tokens(json_compact);
        let toon_tokens = self.count_tokens(toon);

        let token_savings = json_tokens as i64 - toon_tokens as i64;
        let token_savings_vs_compact = json_compact_tokens as i64 - toon_tokens as i64;

        let reduction_percent = if json_tokens > 0 {
            (token_savings as f64 / json_tokens as f64) * 100.0
        } else {
            0.0
        };

        let reduction_percent_vs_compact = if json_compact_tokens > 0 {
            (token_savings_vs_compact as f64 / json_compact_tokens as f64) * 100.0
        } else {
            0.0
        };

        TokenAnalysis {
            source_tokens,
            json_tokens,
            json_compact_tokens,
            toon_tokens,
            token_savings,
            token_savings_vs_compact,
            reduction_percent,
            reduction_percent_vs_compact,
            breakdown: self.analyze_breakdown(toon),
        }
    }

    /// Analyze only TOON vs JSON formats (without source)
    pub fn analyze_formats(
        &self,
        json_pretty: &str,
        json_compact: &str,
        toon: &str,
    ) -> TokenAnalysis {
        let json_tokens = self.count_tokens(json_pretty);
        let json_compact_tokens = self.count_tokens(json_compact);
        let toon_tokens = self.count_tokens(toon);

        let token_savings = json_tokens as i64 - toon_tokens as i64;
        let token_savings_vs_compact = json_compact_tokens as i64 - toon_tokens as i64;

        let reduction_percent = if json_tokens > 0 {
            (token_savings as f64 / json_tokens as f64) * 100.0
        } else {
            0.0
        };

        let reduction_percent_vs_compact = if json_compact_tokens > 0 {
            (token_savings_vs_compact as f64 / json_compact_tokens as f64) * 100.0
        } else {
            0.0
        };

        TokenAnalysis {
            source_tokens: 0,
            json_tokens,
            json_compact_tokens,
            toon_tokens,
            token_savings,
            token_savings_vs_compact,
            reduction_percent,
            reduction_percent_vs_compact,
            breakdown: self.analyze_breakdown(toon),
        }
    }

    /// Analyze breakdown of TOON content
    fn analyze_breakdown(&self, toon: &str) -> TokenBreakdown {
        let mut breakdown = TokenBreakdown::default();

        for line in toon.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                breakdown.whitespace += 1;
                continue;
            }

            // Check if line has a field name (contains ':')
            if let Some(colon_pos) = trimmed.find(':') {
                let field_part = &trimmed[..colon_pos];
                let value_part = &trimmed[colon_pos + 1..];

                breakdown.field_names += self.count_tokens(field_part);
                breakdown.structural += 1; // The colon
                breakdown.values += self.count_tokens(value_part);
            } else {
                // Likely a value-only line (e.g., in arrays)
                breakdown.values += self.count_tokens(trimmed);
            }

            // Count leading whitespace
            let leading_ws = line.len() - line.trim_start().len();
            if leading_ws > 0 {
                breakdown.whitespace += (leading_ws + 4) / 5;
            }
        }

        breakdown
    }
}

/// Split a compound identifier into parts
fn split_identifier(word: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;

    let chars: Vec<char> = word.chars().collect();
    for i in 1..chars.len() {
        // Split on underscore
        if chars[i] == '_' {
            if start < i {
                parts.push(&word[start..i]);
            }
            start = i + 1;
        }
        // Split on camelCase boundary
        else if chars[i].is_uppercase() && chars[i - 1].is_lowercase() {
            parts.push(&word[start..i]);
            start = i;
        }
    }

    if start < word.len() {
        parts.push(&word[start..]);
    }

    if parts.is_empty() {
        vec![word]
    } else {
        parts
    }
}

/// Format token analysis as a human-readable report
pub fn format_analysis_report(analysis: &TokenAnalysis, include_compact: bool) -> String {
    let mut report = String::new();

    report.push_str("═══════════════════════════════════════════════════════\n");
    report.push_str("                   TOKEN ANALYSIS REPORT                \n");
    report.push_str("═══════════════════════════════════════════════════════\n\n");

    if analysis.source_tokens > 0 {
        report.push_str(&format!(
            "Source Code:       {:>6} tokens\n",
            analysis.source_tokens
        ));
    }
    report.push_str(&format!(
        "JSON (pretty):     {:>6} tokens\n",
        analysis.json_tokens
    ));
    if include_compact {
        report.push_str(&format!(
            "JSON (compact):    {:>6} tokens\n",
            analysis.json_compact_tokens
        ));
    }
    report.push_str(&format!(
        "TOON Format:       {:>6} tokens\n",
        analysis.toon_tokens
    ));
    report.push_str("\n───────────────────────────────────────────────────────\n");

    // Savings vs pretty JSON
    report.push_str("vs JSON (pretty):\n");
    if analysis.token_savings > 0 {
        report.push_str(&format!(
            "  Savings:         {:>6} tokens ({:.1}% reduction)\n",
            analysis.token_savings, analysis.reduction_percent
        ));
    } else if analysis.token_savings < 0 {
        report.push_str(&format!(
            "  Overhead:        {:>6} tokens ({:.1}% increase)\n",
            -analysis.token_savings, -analysis.reduction_percent
        ));
    } else {
        report.push_str("  Savings:              0 tokens (no change)\n");
    }

    // Savings vs compact JSON
    if include_compact {
        report.push_str("\nvs JSON (compact):\n");
        if analysis.token_savings_vs_compact > 0 {
            report.push_str(&format!(
                "  Savings:         {:>6} tokens ({:.1}% reduction)\n",
                analysis.token_savings_vs_compact, analysis.reduction_percent_vs_compact
            ));
        } else if analysis.token_savings_vs_compact < 0 {
            report.push_str(&format!(
                "  Overhead:        {:>6} tokens ({:.1}% increase)\n",
                -analysis.token_savings_vs_compact, -analysis.reduction_percent_vs_compact
            ));
        } else {
            report.push_str("  Savings:              0 tokens (no change)\n");
        }
    }

    report.push_str("\n───────────────────────────────────────────────────────\n");
    report.push_str("TOON Breakdown:\n");
    report.push_str(&format!(
        "  Field Names:     {:>6} tokens\n",
        analysis.breakdown.field_names
    ));
    report.push_str(&format!(
        "  Values:          {:>6} tokens\n",
        analysis.breakdown.values
    ));
    report.push_str(&format!(
        "  Structural:      {:>6} tokens\n",
        analysis.breakdown.structural
    ));
    report.push_str(&format!(
        "  Whitespace:      {:>6} tokens\n",
        analysis.breakdown.whitespace
    ));

    report.push_str("═══════════════════════════════════════════════════════\n");

    report
}

/// Format a compact single-line summary
pub fn format_analysis_compact(analysis: &TokenAnalysis, include_compact: bool) -> String {
    if include_compact {
        format!(
            "tokens: json_pretty={} json_compact={} toon={} | saved_vs_pretty={} ({:.1}%) saved_vs_compact={} ({:.1}%)",
            analysis.json_tokens,
            analysis.json_compact_tokens,
            analysis.toon_tokens,
            analysis.token_savings,
            analysis.reduction_percent,
            analysis.token_savings_vs_compact,
            analysis.reduction_percent_vs_compact
        )
    } else {
        format!(
            "tokens: json={} toon={} saved={} ({:.1}%)",
            analysis.json_tokens,
            analysis.toon_tokens,
            analysis.token_savings,
            analysis.reduction_percent
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_token_count() {
        let analyzer = TokenAnalyzer::new();

        // Simple text
        assert!(analyzer.count_tokens("hello") >= 1);
        assert!(analyzer.count_tokens("hello world") >= 2);
    }

    #[test]
    fn test_json_structure() {
        let analyzer = TokenAnalyzer::new();

        let json = r#"{"name": "test", "value": 123}"#;
        let tokens = analyzer.count_tokens(json);

        // JSON has lots of structural tokens (braces, quotes, colons)
        assert!(tokens >= 8);
    }

    #[test]
    fn test_toon_structure() {
        let analyzer = TokenAnalyzer::new();

        let toon = "name: test\nvalue: 123";
        let tokens = analyzer.count_tokens(toon);

        // TOON should have fewer structural tokens
        assert!(tokens >= 4);
    }

    #[test]
    fn test_toon_more_efficient() {
        let analyzer = TokenAnalyzer::new();

        let json = r#"{"file": "test.tsx", "language": "tsx", "symbol": "App"}"#;
        let toon = "file: test.tsx\nlanguage: tsx\nsymbol: App";

        let json_tokens = analyzer.count_tokens(json);
        let toon_tokens = analyzer.count_tokens(toon);

        // TOON should be more efficient
        assert!(toon_tokens <= json_tokens);
    }

    #[test]
    fn test_analyze_formats() {
        let analyzer = TokenAnalyzer::new();

        let json_pretty = "{\n  \"name\": \"test\"\n}";
        let json_compact = r#"{"name":"test"}"#;
        let toon = "name: test";

        let analysis = analyzer.analyze_formats(json_pretty, json_compact, toon);

        assert!(analysis.json_tokens > 0);
        assert!(analysis.json_compact_tokens > 0);
        assert!(analysis.toon_tokens > 0);
        assert!(analysis.toon_tokens <= analysis.json_tokens);
        assert!(analysis.json_compact_tokens <= analysis.json_tokens);
    }

    #[test]
    fn test_split_identifier() {
        assert_eq!(split_identifier("camelCase"), vec!["camel", "Case"]);
        assert_eq!(split_identifier("snake_case"), vec!["snake", "case"]);
        assert_eq!(split_identifier("simple"), vec!["simple"]);
        assert_eq!(split_identifier("XMLParser"), vec!["XMLParser"]);
    }

    #[test]
    fn test_number_tokens() {
        let analyzer = TokenAnalyzer::new();

        // Numbers should be ~3-4 digits per token
        assert_eq!(analyzer.count_tokens("123"), 1);
        assert_eq!(analyzer.count_tokens("123456"), 2);
        assert_eq!(analyzer.count_tokens("123456789012"), 3);
    }

    #[test]
    fn test_format_report() {
        let analysis = TokenAnalysis {
            source_tokens: 100,
            json_tokens: 50,
            json_compact_tokens: 40,
            toon_tokens: 30,
            token_savings: 20,
            token_savings_vs_compact: 10,
            reduction_percent: 40.0,
            reduction_percent_vs_compact: 25.0,
            breakdown: TokenBreakdown {
                field_names: 10,
                values: 15,
                structural: 3,
                whitespace: 2,
            },
        };

        let report = format_analysis_report(&analysis, true);
        assert!(report.contains("JSON (pretty):"));
        assert!(report.contains("JSON (compact):"));
        assert!(report.contains("TOON Format:"));
        assert!(report.contains("vs JSON (pretty):"));
        assert!(report.contains("vs JSON (compact):"));
    }
}
