//! Validate command handler - Quality audits (duplicates)

use std::fs;
use std::io::{BufRead, BufReader};

use crate::cache::CacheDir;
use crate::cli::{OutputFormat, ValidateArgs};
use crate::commands::CommandContext;
use crate::error::{McpDiffError, Result};
use crate::{DuplicateDetector, FunctionSignature};

/// Run the validate command - find duplicates and consolidation opportunities
pub fn run_validate(args: &ValidateArgs, ctx: &CommandContext) -> Result<String> {
    let repo_dir = std::env::current_dir().map_err(|e| McpDiffError::FileNotFound {
        path: format!("current directory: {}", e),
    })?;
    let cache = CacheDir::for_repo(&repo_dir)?;

    // If duplicates flag is set or a hash is provided, run duplicate detection
    if args.duplicates {
        if let Some(ref target) = args.target {
            // Check if target looks like a hash (for single symbol duplicate check)
            if target.contains(':') || target.len() >= 16 {
                return run_check_duplicates(target, args.threshold, &cache, ctx);
            }
        }
    }

    // Run full duplicate scan with optional file/module filter
    run_find_duplicates(args, &cache, ctx)
}

/// Load function signatures from cache
fn load_signatures(cache: &CacheDir) -> Result<Vec<FunctionSignature>> {
    let sig_path = cache.signature_index_path();
    if !sig_path.exists() {
        return Err(McpDiffError::FileNotFound {
            path: "Signature index not found. Run `semfora index generate` first.".to_string(),
        });
    }

    let file = fs::File::open(&sig_path)?;
    let reader = BufReader::new(file);

    let mut signatures = Vec::new();
    for line in reader.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(sig) = serde_json::from_str::<FunctionSignature>(&line) {
            signatures.push(sig);
        }
    }

    Ok(signatures)
}

/// Find all duplicates in the codebase
fn run_find_duplicates(args: &ValidateArgs, cache: &CacheDir, ctx: &CommandContext) -> Result<String> {
    if !cache.exists() {
        return Err(McpDiffError::GitError {
            message: "No index found. Run `semfora index generate` first.".to_string(),
        });
    }

    eprintln!("Loading function signatures...");
    let mut signatures = load_signatures(cache)?;

    // Filter by target if specified (file path or module name)
    if let Some(ref target) = args.target {
        let target_lower = target.to_lowercase();
        signatures.retain(|sig| sig.file.to_lowercase().contains(&target_lower));

        if signatures.is_empty() {
            return Err(McpDiffError::FileNotFound {
                path: format!("No symbols found matching: {}", target),
            });
        }
    }

    if signatures.is_empty() {
        return Ok("No function signatures found in index.".to_string());
    }

    eprintln!("Analyzing {} signatures for duplicates...", signatures.len());

    let exclude_boilerplate = !args.include_boilerplate;
    let detector = DuplicateDetector::new(args.threshold)
        .with_boilerplate_exclusion(exclude_boilerplate);

    let clusters = detector.find_all_clusters(&signatures);

    let mut output = String::new();

    match ctx.format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "threshold": args.threshold,
                "boilerplate_excluded": exclude_boilerplate,
                "total_signatures": signatures.len(),
                "filter": args.target,
                "clusters": clusters.len(),
                "total_duplicates": clusters.iter().map(|c| c.duplicates.len()).sum::<usize>(),
                "cluster_details": clusters.iter().take(args.limit.min(50)).map(|c| serde_json::json!({
                    "primary": c.primary.name,
                    "primary_file": c.primary.file,
                    "primary_hash": c.primary.hash,
                    "duplicate_count": c.duplicates.len(),
                    "duplicates": c.duplicates.iter().take(5).map(|d| serde_json::json!({
                        "name": d.symbol.name,
                        "file": d.symbol.file,
                        "similarity": d.similarity,
                        "kind": format!("{:?}", d.kind)
                    })).collect::<Vec<_>>()
                })).collect::<Vec<_>>()
            });
            output = serde_json::to_string_pretty(&json).unwrap_or_default();
        }
        OutputFormat::Toon => {
            output.push_str("═══════════════════════════════════════════\n");
            if let Some(ref target) = args.target {
                output.push_str(&format!("  DUPLICATE ANALYSIS: {}\n", target));
            } else {
                output.push_str("  DUPLICATE ANALYSIS\n");
            }
            output.push_str("═══════════════════════════════════════════\n\n");

            output.push_str(&format!("Threshold: {:.0}%\n", args.threshold * 100.0));
            output.push_str(&format!("Boilerplate excluded: {}\n", exclude_boilerplate));
            output.push_str(&format!("Signatures analyzed: {}\n", signatures.len()));
            output.push_str(&format!("Clusters found: {}\n\n", clusters.len()));

            if clusters.is_empty() {
                output.push_str("No duplicate clusters found above threshold.\n");
                return Ok(output);
            }

            let total_duplicates: usize = clusters.iter().map(|c| c.duplicates.len()).sum();
            output.push_str(&format!("Total duplicate functions: {}\n\n", total_duplicates));

            for (i, cluster) in clusters.iter().take(args.limit.min(20)).enumerate() {
                output.push_str("───────────────────────────────────────────\n");
                output.push_str(&format!("Cluster {} ({} duplicates)\n", i + 1, cluster.duplicates.len()));
                output.push_str(&format!("Primary: {}\n", cluster.primary.name));
                output.push_str(&format!("  file: {}\n", cluster.primary.file));
                output.push_str(&format!("  hash: {}\n", cluster.primary.hash));

                output.push_str("Duplicates:\n");
                for dup in cluster.duplicates.iter().take(5) {
                    let kind_str = match dup.kind {
                        crate::DuplicateKind::Exact => "EXACT",
                        crate::DuplicateKind::Near => "NEAR",
                        crate::DuplicateKind::Divergent => "DIVERGENT",
                    };
                    output.push_str(&format!(
                        "  - {} [{} {:.0}%]\n    {}\n",
                        dup.symbol.name, kind_str, dup.similarity * 100.0, dup.symbol.file
                    ));
                }
                if cluster.duplicates.len() > 5 {
                    output.push_str(&format!("  ... and {} more\n", cluster.duplicates.len() - 5));
                }
                output.push('\n');
            }

            if clusters.len() > args.limit.min(20) {
                output.push_str(&format!("\n... showing {} of {} clusters\n", args.limit.min(20), clusters.len()));
            }
        }
    }

    Ok(output)
}

/// Check duplicates for a specific symbol by hash
fn run_check_duplicates(hash: &str, threshold: f64, cache: &CacheDir, ctx: &CommandContext) -> Result<String> {
    // Load all signatures
    let signatures = load_signatures(cache)?;

    // Find the signature with matching hash
    let target_sig = signatures
        .iter()
        .find(|s| s.symbol_hash == hash || s.symbol_hash.starts_with(hash))
        .ok_or_else(|| McpDiffError::FileNotFound {
            path: format!("Symbol not found: {}", hash),
        })?;

    // Find duplicates for this specific symbol using DuplicateDetector
    let detector = DuplicateDetector::new(threshold);
    let mut duplicates = detector.find_duplicates(target_sig, &signatures);

    // Sort by similarity descending
    duplicates.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));

    let mut output = String::new();

    match ctx.format {
        OutputFormat::Json => {
            let json = serde_json::json!({
                "symbol": target_sig.name,
                "hash": target_sig.symbol_hash,
                "file": target_sig.file,
                "threshold": threshold,
                "duplicates": duplicates.iter().map(|dup| serde_json::json!({
                    "name": dup.symbol.name,
                    "hash": dup.symbol.hash,
                    "file": dup.symbol.file,
                    "similarity": dup.similarity,
                    "kind": format!("{:?}", dup.kind)
                })).collect::<Vec<_>>(),
                "count": duplicates.len()
            });
            output = serde_json::to_string_pretty(&json).unwrap_or_default();
        }
        OutputFormat::Toon => {
            output.push_str(&format!("symbol: {}\n", target_sig.name));
            output.push_str(&format!("hash: {}\n", target_sig.symbol_hash));
            output.push_str(&format!("file: {}\n", target_sig.file));
            output.push_str(&format!("threshold: {:.0}%\n\n", threshold * 100.0));
            output.push_str(&format!("duplicates[{}]:\n", duplicates.len()));

            if duplicates.is_empty() {
                output.push_str("  (no duplicates found)\n");
            } else {
                for dup in &duplicates {
                    output.push_str(&format!(
                        "  - {} ({:.0}%)\n    {}\n",
                        dup.symbol.name, dup.similarity * 100.0, dup.symbol.file
                    ));
                }
            }
        }
    }

    Ok(output)
}
