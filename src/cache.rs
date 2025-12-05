//! Cache storage module for sharded semantic index
//!
//! Provides XDG-compliant cache directory management and repo hashing
//! for storing sharded semantic IR that can be queried by AI agents.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::git;
use crate::overlay::{LayerKind, LayeredIndex, Overlay};
use crate::schema::{fnv1a_hash, SCHEMA_VERSION};

/// Metadata for cached files to detect staleness
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMeta {
    /// Schema version for compatibility
    pub schema_version: String,

    /// When this cache was generated
    pub generated_at: String,

    /// Source files that contributed to this cache entry
    pub source_files: Vec<SourceFileInfo>,

    /// Indexing status (for progressive indexing)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub indexing_status: Option<IndexingStatus>,
}

/// Information about a source file for staleness detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceFileInfo {
    /// Relative path from repo root
    pub path: String,

    /// File modification time (Unix timestamp)
    pub mtime: u64,

    /// File size in bytes (for quick change detection)
    pub size: u64,
}

impl SourceFileInfo {
    /// Create from a file path
    pub fn from_path(path: &Path, repo_root: &Path) -> Option<Self> {
        let metadata = fs::metadata(path).ok()?;
        let mtime = metadata
            .modified()
            .ok()?
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()?
            .as_secs();

        let relative_path = path
            .strip_prefix(repo_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        Some(Self {
            path: relative_path,
            mtime,
            size: metadata.len(),
        })
    }

    /// Check if the source file has changed
    pub fn is_stale(&self, repo_root: &Path) -> bool {
        let full_path = repo_root.join(&self.path);
        match fs::metadata(&full_path) {
            Ok(metadata) => {
                let current_mtime = metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);

                // Stale if mtime changed or size changed
                current_mtime != self.mtime || metadata.len() != self.size
            }
            Err(_) => true, // File deleted or inaccessible = stale
        }
    }
}

/// Progress status for ongoing indexing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatus {
    /// Whether indexing is in progress
    pub in_progress: bool,

    /// Number of files indexed so far
    pub files_indexed: usize,

    /// Total number of files to index
    pub files_total: usize,

    /// Percentage complete (0-100)
    pub percent: u8,

    /// Estimated seconds remaining
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta_seconds: Option<u32>,

    /// Modules that are ready to query
    pub modules_ready: Vec<String>,

    /// Modules still being indexed
    pub modules_pending: Vec<String>,
}

impl Default for IndexingStatus {
    fn default() -> Self {
        Self {
            in_progress: false,
            files_indexed: 0,
            files_total: 0,
            percent: 0,
            eta_seconds: None,
            modules_ready: Vec::new(),
            modules_pending: Vec::new(),
        }
    }
}

impl CacheMeta {
    /// Create a new cache metadata entry
    pub fn new(source_files: Vec<SourceFileInfo>) -> Self {
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            generated_at: chrono::Utc::now().to_rfc3339(),
            source_files,
            indexing_status: None,
        }
    }

    /// Create metadata for a single file
    pub fn for_file(path: &Path, repo_root: &Path) -> Self {
        let source_files = SourceFileInfo::from_path(path, repo_root)
            .map(|f| vec![f])
            .unwrap_or_default();
        Self::new(source_files)
    }

    /// Check if any source file is stale
    pub fn is_stale(&self, repo_root: &Path) -> bool {
        self.source_files.iter().any(|f| f.is_stale(repo_root))
    }

    /// Check if schema version is compatible
    pub fn is_compatible(&self) -> bool {
        self.schema_version == SCHEMA_VERSION
    }
}

/// Cache directory structure manager
pub struct CacheDir {
    /// Root of the cache for this repo
    pub root: PathBuf,

    /// Path to the repository being indexed
    pub repo_root: PathBuf,

    /// Repo hash (for identification)
    pub repo_hash: String,
}

impl CacheDir {
    /// Create a cache directory for a repository
    pub fn for_repo(repo_path: &Path) -> Result<Self> {
        let repo_root = repo_path.canonicalize().unwrap_or_else(|_| repo_path.to_path_buf());
        let repo_hash = compute_repo_hash(&repo_root);
        let cache_base = get_cache_base_dir();
        let root = cache_base.join(&repo_hash);

        Ok(Self {
            root,
            repo_root,
            repo_hash,
        })
    }

    /// Initialize the cache directory structure
    pub fn init(&self) -> Result<()> {
        // Create main directories
        fs::create_dir_all(&self.root)?;
        fs::create_dir_all(self.modules_dir())?;
        fs::create_dir_all(self.symbols_dir())?;
        fs::create_dir_all(self.graphs_dir())?;
        fs::create_dir_all(self.diffs_dir())?;
        fs::create_dir_all(self.layers_dir())?;

        Ok(())
    }

    /// Check if the cache exists and is initialized
    pub fn exists(&self) -> bool {
        self.root.exists() && self.repo_overview_path().exists()
    }

    // ========== Path accessors ==========

    /// Path to repo_overview.toon
    pub fn repo_overview_path(&self) -> PathBuf {
        self.root.join("repo_overview.toon")
    }

    /// Path to modules directory
    pub fn modules_dir(&self) -> PathBuf {
        self.root.join("modules")
    }

    /// Path to a specific module file
    pub fn module_path(&self, module_name: &str) -> PathBuf {
        self.modules_dir().join(format!("{}.toon", sanitize_filename(module_name)))
    }

    /// Path to symbols directory
    pub fn symbols_dir(&self) -> PathBuf {
        self.root.join("symbols")
    }

    /// Path to a specific symbol file
    pub fn symbol_path(&self, symbol_hash: &str) -> PathBuf {
        self.symbols_dir().join(format!("{}.toon", symbol_hash))
    }

    /// Path to graphs directory
    pub fn graphs_dir(&self) -> PathBuf {
        self.root.join("graphs")
    }

    /// Path to call graph
    pub fn call_graph_path(&self) -> PathBuf {
        self.graphs_dir().join("call_graph.toon")
    }

    /// Path to import graph
    pub fn import_graph_path(&self) -> PathBuf {
        self.graphs_dir().join("import_graph.toon")
    }

    /// Path to module graph
    pub fn module_graph_path(&self) -> PathBuf {
        self.graphs_dir().join("module_graph.toon")
    }

    /// Path to diffs directory
    pub fn diffs_dir(&self) -> PathBuf {
        self.root.join("diffs")
    }

    /// Path to a specific diff file
    pub fn diff_path(&self, commit_sha: &str) -> PathBuf {
        self.diffs_dir().join(format!("commit_{}.toon", commit_sha))
    }

    // ========== Layer paths (SEM-45) ==========

    /// Path to layers directory
    pub fn layers_dir(&self) -> PathBuf {
        self.root.join("layers")
    }

    /// Path to a specific layer file
    ///
    /// AI layer is not persisted - returns None for LayerKind::AI
    pub fn layer_path(&self, kind: LayerKind) -> Option<PathBuf> {
        match kind {
            LayerKind::AI => None, // AI layer is ephemeral
            _ => Some(self.layers_dir().join(format!("{}.json", kind.as_str()))),
        }
    }

    /// Path to layered index metadata file
    pub fn layer_meta_path(&self) -> PathBuf {
        self.layers_dir().join("meta.json")
    }

    /// Check if cached layers exist
    pub fn has_cached_layers(&self) -> bool {
        self.layers_dir().exists() && self.layer_path(LayerKind::Base).map(|p| p.exists()).unwrap_or(false)
    }

    // ========== Utility methods ==========

    /// List all module names in the cache
    pub fn list_modules(&self) -> Vec<String> {
        fs::read_dir(self.modules_dir())
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().map(|e| e == "toon").unwrap_or(false) {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// List all symbol hashes in the cache
    pub fn list_symbols(&self) -> Vec<String> {
        fs::read_dir(self.symbols_dir())
            .into_iter()
            .flatten()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                if path.extension().map(|e| e == "toon").unwrap_or(false) {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .map(|s| s.to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Get cache size in bytes
    pub fn size(&self) -> u64 {
        dir_size(&self.root)
    }

    /// Clear the cache
    pub fn clear(&self) -> Result<()> {
        if self.root.exists() {
            fs::remove_dir_all(&self.root)?;
        }
        Ok(())
    }

    // ========== Query-Driven API (v1) ==========

    /// Path to the symbol index file (JSONL format)
    pub fn symbol_index_path(&self) -> PathBuf {
        self.root.join("symbol_index.jsonl")
    }

    /// Check if symbol index exists
    pub fn has_symbol_index(&self) -> bool {
        self.symbol_index_path().exists()
    }

    /// Search symbol index with filters
    /// Returns lightweight entries matching the query
    pub fn search_symbols(
        &self,
        query: &str,
        module_filter: Option<&str>,
        kind_filter: Option<&str>,
        risk_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SymbolIndexEntry>> {
        use std::io::BufRead;

        let index_path = self.symbol_index_path();
        if !index_path.exists() {
            return Err(crate::McpDiffError::FileNotFound {
                path: index_path.display().to_string(),
            });
        }

        let file = fs::File::open(&index_path)?;
        let reader = std::io::BufReader::new(file);
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: SymbolIndexEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue, // Skip malformed lines
            };

            // Match query against symbol name (case-insensitive, partial)
            if !entry.symbol.to_lowercase().contains(&query_lower) {
                continue;
            }

            // Apply optional filters
            if let Some(m) = module_filter {
                if entry.module != m {
                    continue;
                }
            }
            if let Some(k) = kind_filter {
                if entry.kind != k {
                    continue;
                }
            }
            if let Some(r) = risk_filter {
                if entry.risk != r {
                    continue;
                }
            }

            results.push(entry);

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    /// List symbols in a module (lightweight index only)
    pub fn list_module_symbols(
        &self,
        module: &str,
        kind_filter: Option<&str>,
        risk_filter: Option<&str>,
        limit: usize,
    ) -> Result<Vec<SymbolIndexEntry>> {
        use std::io::BufRead;

        let index_path = self.symbol_index_path();
        if !index_path.exists() {
            return Err(crate::McpDiffError::FileNotFound {
                path: index_path.display().to_string(),
            });
        }

        let file = fs::File::open(&index_path)?;
        let reader = std::io::BufReader::new(file);
        let mut results = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let entry: SymbolIndexEntry = match serde_json::from_str(&line) {
                Ok(e) => e,
                Err(_) => continue,
            };

            // Must match module
            if entry.module != module {
                continue;
            }

            // Apply optional filters
            if let Some(k) = kind_filter {
                if entry.kind != k {
                    continue;
                }
            }
            if let Some(r) = risk_filter {
                if entry.risk != r {
                    continue;
                }
            }

            results.push(entry);

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    // ========== Layer persistence (SEM-45) ==========

    /// Save a single layer overlay to cache
    ///
    /// Uses atomic write (temp file + rename) for crash safety.
    /// AI layer is not persisted and returns Ok(()) immediately.
    pub fn save_layer(&self, overlay: &Overlay) -> Result<()> {
        let kind = match overlay.meta.kind {
            Some(k) => k,
            None => return Err(crate::McpDiffError::ExtractionFailure {
                message: "Cannot save overlay with unknown layer kind".to_string(),
            }),
        };

        let path = match self.layer_path(kind) {
            Some(p) => p,
            None => return Ok(()), // AI layer - skip
        };

        // Ensure layers directory exists
        fs::create_dir_all(self.layers_dir())?;

        // Atomic write: write to temp file, then rename
        let temp_path = path.with_extension("json.tmp");
        let json = serde_json::to_string_pretty(overlay).map_err(|e| {
            crate::McpDiffError::ExtractionFailure {
                message: format!("Failed to serialize {} layer: {}", kind, e),
            }
        })?;

        fs::write(&temp_path, &json)?;
        fs::rename(&temp_path, &path)?;

        Ok(())
    }

    /// Load a single layer overlay from cache
    ///
    /// Returns None if layer file doesn't exist or AI layer is requested.
    pub fn load_layer(&self, kind: LayerKind) -> Result<Option<Overlay>> {
        let path = match self.layer_path(kind) {
            Some(p) => p,
            None => return Ok(None), // AI layer - not persisted
        };

        if !path.exists() {
            return Ok(None);
        }

        let json = fs::read_to_string(&path)?;
        let overlay: Overlay = serde_json::from_str(&json).map_err(|e| {
            crate::McpDiffError::ExtractionFailure {
                message: format!("Failed to deserialize {} layer: {}", kind, e),
            }
        })?;

        Ok(Some(overlay))
    }

    /// Save a full LayeredIndex to cache
    ///
    /// Saves base, branch, and working layers. AI layer is ephemeral.
    pub fn save_layered_index(&self, index: &LayeredIndex) -> Result<()> {
        // Save each persistent layer
        self.save_layer(&index.base)?;
        self.save_layer(&index.branch)?;
        self.save_layer(&index.working)?;
        // AI layer is not saved (ephemeral)

        // Save metadata
        let meta = LayeredIndexMeta {
            schema_version: SCHEMA_VERSION.to_string(),
            saved_at: chrono::Utc::now().to_rfc3339(),
            base_indexed_sha: index.base.meta.indexed_sha.clone(),
            branch_indexed_sha: index.branch.meta.indexed_sha.clone(),
            merge_base: index.base.meta.merge_base_sha.clone(),
        };

        let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| {
            crate::McpDiffError::ExtractionFailure {
                message: format!("Failed to serialize layer meta: {}", e),
            }
        })?;

        fs::write(self.layer_meta_path(), &meta_json)?;

        Ok(())
    }

    /// Load a full LayeredIndex from cache
    ///
    /// Returns None if layers haven't been cached yet.
    /// AI layer is always initialized empty.
    pub fn load_layered_index(&self) -> Result<Option<LayeredIndex>> {
        if !self.has_cached_layers() {
            return Ok(None);
        }

        // Load each layer
        let base = match self.load_layer(LayerKind::Base)? {
            Some(o) => o,
            None => return Ok(None),
        };

        let branch = self.load_layer(LayerKind::Branch)?.unwrap_or_else(|| Overlay::new(LayerKind::Branch));
        let working = self.load_layer(LayerKind::Working)?.unwrap_or_else(|| Overlay::new(LayerKind::Working));
        let ai = Overlay::new(LayerKind::AI); // AI is always fresh

        Ok(Some(LayeredIndex {
            base,
            branch,
            working,
            ai,
        }))
    }

    /// Clear all cached layers
    pub fn clear_layers(&self) -> Result<()> {
        let layers_dir = self.layers_dir();
        if layers_dir.exists() {
            fs::remove_dir_all(&layers_dir)?;
        }
        Ok(())
    }

    // ========== Layer staleness detection (SEM-45) ==========

    /// Check if a cached layer is stale
    ///
    /// Staleness rules:
    /// - Base: indexed_sha != current HEAD of main/master
    /// - Branch: indexed_sha != current branch HEAD
    /// - Working: any tracked file changed (mtime/size)
    /// - AI: always fresh (not persisted)
    pub fn is_layer_stale(&self, kind: LayerKind) -> Result<bool> {
        let overlay = match self.load_layer(kind)? {
            Some(o) => o,
            None => return Ok(true), // No cached layer = stale
        };

        match kind {
            LayerKind::Base => self.is_base_layer_stale(&overlay),
            LayerKind::Branch => self.is_branch_layer_stale(&overlay),
            LayerKind::Working => self.is_working_layer_stale(&overlay),
            LayerKind::AI => Ok(false), // AI layer is never persisted, always fresh in memory
        }
    }

    /// Check if base layer is stale (indexed SHA != main/master HEAD)
    fn is_base_layer_stale(&self, overlay: &Overlay) -> Result<bool> {
        let indexed_sha = match &overlay.meta.indexed_sha {
            Some(sha) => sha,
            None => return Ok(true), // No indexed SHA = stale
        };

        // Get the current base branch HEAD
        let base_branch = git::detect_base_branch(Some(&self.repo_root))?;
        let current_sha = get_ref_sha(&base_branch, Some(&self.repo_root))?;

        Ok(indexed_sha != &current_sha)
    }

    /// Check if branch layer is stale (indexed SHA != branch HEAD, or merge-base changed)
    fn is_branch_layer_stale(&self, overlay: &Overlay) -> Result<bool> {
        let indexed_sha = match &overlay.meta.indexed_sha {
            Some(sha) => sha,
            None => return Ok(true), // No indexed SHA = stale
        };

        // Check if branch HEAD has moved
        let current_sha = get_ref_sha("HEAD", Some(&self.repo_root))?;
        if indexed_sha != &current_sha {
            return Ok(true);
        }

        // Check if merge-base has changed (rebase scenario)
        if let Some(stored_merge_base) = &overlay.meta.merge_base_sha {
            let base_branch = git::detect_base_branch(Some(&self.repo_root))?;
            let current_merge_base = git::get_merge_base("HEAD", &base_branch, Some(&self.repo_root))?;
            if stored_merge_base != &current_merge_base {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if working layer is stale (any tracked file has changed)
    fn is_working_layer_stale(&self, overlay: &Overlay) -> Result<bool> {
        // Working layer tracks files via symbols_by_file
        // Check if any tracked file's mtime/size has changed
        for file_path in overlay.symbols_by_file.keys() {
            let full_path = self.repo_root.join(file_path);
            match fs::metadata(&full_path) {
                Ok(meta) => {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    // If the layer was updated before the file was modified, it's stale
                    if mtime > overlay.meta.updated_at {
                        return Ok(true);
                    }
                }
                Err(_) => {
                    // File deleted or inaccessible = stale
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    /// Check if entire LayeredIndex cache is stale
    pub fn is_layered_index_stale(&self) -> Result<bool> {
        // If no cache exists, it's "stale" (needs to be created)
        if !self.has_cached_layers() {
            return Ok(true);
        }

        // Base layer staleness is most critical - rebuild if base moved
        if self.is_layer_stale(LayerKind::Base)? {
            return Ok(true);
        }

        // Branch and working layers can be rebuilt incrementally,
        // but for now we consider the whole index stale if any layer is stale
        if self.is_layer_stale(LayerKind::Branch)? {
            return Ok(true);
        }

        if self.is_layer_stale(LayerKind::Working)? {
            return Ok(true);
        }

        Ok(false)
    }
}

/// Get the SHA for a git reference
fn get_ref_sha(ref_name: &str, cwd: Option<&Path>) -> Result<String> {
    git::git_command(&["rev-parse", ref_name], cwd)
}

/// Metadata for cached LayeredIndex
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayeredIndexMeta {
    /// Schema version for compatibility
    pub schema_version: String,

    /// When the layers were saved
    pub saved_at: String,

    /// Git SHA that base layer was indexed at
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_indexed_sha: Option<String>,

    /// Git SHA that branch layer was indexed at
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_indexed_sha: Option<String>,

    /// Merge base SHA (where branch diverged from base)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merge_base: Option<String>,
}

/// Lightweight symbol index entry for query-driven access
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SymbolIndexEntry {
    /// Symbol name
    #[serde(rename = "s")]
    pub symbol: String,

    /// Symbol hash (for get_symbol lookup)
    #[serde(rename = "h")]
    pub hash: String,

    /// Symbol kind (fn, struct, component, enum, trait, etc.)
    #[serde(rename = "k")]
    pub kind: String,

    /// Module name
    #[serde(rename = "m")]
    pub module: String,

    /// File path (relative to repo root)
    #[serde(rename = "f")]
    pub file: String,

    /// Line range (e.g., "45-89")
    #[serde(rename = "l")]
    pub lines: String,

    /// Risk level (high, medium, low)
    #[serde(rename = "r")]
    pub risk: String,
}

/// Get the base cache directory (XDG-compliant)
pub fn get_cache_base_dir() -> PathBuf {
    // Check XDG_CACHE_HOME first
    if let Ok(xdg_cache) = std::env::var("XDG_CACHE_HOME") {
        return PathBuf::from(xdg_cache).join("semfora");
    }

    // Fall back to ~/.cache/semfora
    if let Some(home) = dirs::home_dir() {
        return home.join(".cache").join("semfora");
    }

    // Last resort: temp directory
    std::env::temp_dir().join("semfora")
}

/// Compute a stable hash for a repository
///
/// Prefers git remote URL for consistency across clones,
/// falls back to absolute path.
pub fn compute_repo_hash(repo_path: &Path) -> String {
    // Try to get git remote URL first
    if let Some(remote_url) = get_git_remote_url(repo_path) {
        return format!("{:016x}", fnv1a_hash(&remote_url));
    }

    // Fall back to absolute path
    let canonical = repo_path
        .canonicalize()
        .unwrap_or_else(|_| repo_path.to_path_buf());
    format!("{:016x}", fnv1a_hash(&canonical.to_string_lossy()))
}

/// Get the git remote URL for a repository
fn get_git_remote_url(repo_path: &Path) -> Option<String> {
    let output = Command::new("git")
        .args(["remote", "get-url", "origin"])
        .current_dir(repo_path)
        .output()
        .ok()?;

    if output.status.success() {
        let url = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !url.is_empty() {
            return Some(url);
        }
    }

    None
}

/// Sanitize a string for use as a filename
fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Calculate total size of a directory
fn dir_size(path: &Path) -> u64 {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                dir_size(&path)
            } else {
                fs::metadata(&path).map(|m| m.len()).unwrap_or(0)
            }
        })
        .sum()
}

/// List all cached repositories
pub fn list_cached_repos() -> Vec<(String, PathBuf, u64)> {
    let cache_base = get_cache_base_dir();

    fs::read_dir(&cache_base)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| {
            let path = entry.path();
            let hash = entry.file_name().to_string_lossy().to_string();
            let size = dir_size(&path);
            (hash, path, size)
        })
        .collect()
}

/// Prune caches older than the specified number of days
pub fn prune_old_caches(days: u32) -> Result<usize> {
    let cache_base = get_cache_base_dir();
    let cutoff = SystemTime::now()
        .checked_sub(std::time::Duration::from_secs(days as u64 * 24 * 60 * 60))
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut count = 0;

    for entry in fs::read_dir(&cache_base)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // Check the modification time of repo_overview.toon as proxy for last use
        let overview_path = path.join("repo_overview.toon");
        if let Ok(metadata) = fs::metadata(&overview_path) {
            if let Ok(modified) = metadata.modified() {
                if modified < cutoff {
                    fs::remove_dir_all(&path)?;
                    count += 1;
                }
            }
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_compute_repo_hash_deterministic() {
        let path = Path::new("/tmp/test-repo");
        let hash1 = compute_repo_hash(path);
        let hash2 = compute_repo_hash(path);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 16); // 64-bit hash as hex
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("api"), "api");
        assert_eq!(sanitize_filename("components/ui"), "components_ui");
        assert_eq!(sanitize_filename("src:main"), "src_main");
    }

    #[test]
    fn test_cache_base_dir() {
        let base = get_cache_base_dir();
        assert!(base.to_string_lossy().contains("semfora"));
    }

    #[test]
    fn test_cache_dir_paths() {
        let cache = CacheDir {
            root: PathBuf::from("/tmp/semfora/abc123"),
            repo_root: PathBuf::from("/home/user/project"),
            repo_hash: "abc123".to_string(),
        };

        assert_eq!(
            cache.repo_overview_path(),
            PathBuf::from("/tmp/semfora/abc123/repo_overview.toon")
        );
        assert_eq!(
            cache.module_path("api"),
            PathBuf::from("/tmp/semfora/abc123/modules/api.toon")
        );
        assert_eq!(
            cache.symbol_path("def456"),
            PathBuf::from("/tmp/semfora/abc123/symbols/def456.toon")
        );
    }

    #[test]
    fn test_source_file_info() {
        // Test with current file
        let current_file = Path::new(file!());
        let repo_root = env::current_dir().unwrap();

        if let Some(info) = SourceFileInfo::from_path(current_file, &repo_root) {
            assert!(!info.path.is_empty());
            assert!(info.mtime > 0);
            assert!(info.size > 0);
            assert!(!info.is_stale(&repo_root));
        }
    }

    // ========================================================================
    // Layer Cache Tests (SEM-45)
    // ========================================================================

    #[test]
    fn test_layer_paths() {
        let cache = CacheDir {
            root: PathBuf::from("/tmp/semfora/abc123"),
            repo_root: PathBuf::from("/home/user/project"),
            repo_hash: "abc123".to_string(),
        };

        // Test layers directory path
        assert_eq!(
            cache.layers_dir(),
            PathBuf::from("/tmp/semfora/abc123/layers")
        );

        // Test layer paths for each kind
        assert_eq!(
            cache.layer_path(LayerKind::Base),
            Some(PathBuf::from("/tmp/semfora/abc123/layers/base.json"))
        );
        assert_eq!(
            cache.layer_path(LayerKind::Branch),
            Some(PathBuf::from("/tmp/semfora/abc123/layers/branch.json"))
        );
        assert_eq!(
            cache.layer_path(LayerKind::Working),
            Some(PathBuf::from("/tmp/semfora/abc123/layers/working.json"))
        );
        // AI layer should return None (ephemeral)
        assert_eq!(cache.layer_path(LayerKind::AI), None);

        // Test layer meta path
        assert_eq!(
            cache.layer_meta_path(),
            PathBuf::from("/tmp/semfora/abc123/layers/meta.json")
        );
    }

    #[test]
    fn test_save_and_load_layer() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheDir {
            root: temp_dir.path().to_path_buf(),
            repo_root: temp_dir.path().to_path_buf(),
            repo_hash: "test_hash".to_string(),
        };

        // Create a test overlay
        let mut overlay = Overlay::new(LayerKind::Base);
        overlay.meta.indexed_sha = Some("abc123".to_string());

        // Save the layer
        cache.save_layer(&overlay).expect("Failed to save layer");

        // Verify the file exists
        let path = cache.layer_path(LayerKind::Base).unwrap();
        assert!(path.exists(), "Layer file should exist after save");

        // Load the layer back
        let loaded = cache.load_layer(LayerKind::Base).expect("Failed to load layer");
        assert!(loaded.is_some(), "Should load the saved layer");

        let loaded_overlay = loaded.unwrap();
        assert_eq!(loaded_overlay.meta.indexed_sha, Some("abc123".to_string()));
    }

    #[test]
    fn test_ai_layer_not_saved() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheDir {
            root: temp_dir.path().to_path_buf(),
            repo_root: temp_dir.path().to_path_buf(),
            repo_hash: "test_hash".to_string(),
        };

        // Create an AI overlay
        let overlay = Overlay::new(LayerKind::AI);

        // Attempting to save AI layer should succeed but not create a file
        cache.save_layer(&overlay).expect("Save should succeed for AI layer");

        // The layers directory shouldn't have any ai.json file
        let ai_path = cache.layers_dir().join("ai.json");
        assert!(!ai_path.exists(), "AI layer should not be persisted");

        // Loading AI layer should return None
        let loaded = cache.load_layer(LayerKind::AI).expect("Load should succeed");
        assert!(loaded.is_none(), "Loading AI layer should return None");
    }

    #[test]
    fn test_save_and_load_layered_index() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheDir {
            root: temp_dir.path().to_path_buf(),
            repo_root: temp_dir.path().to_path_buf(),
            repo_hash: "test_hash".to_string(),
        };

        // Create a layered index with some data
        let mut index = LayeredIndex::new();
        index.base.meta.indexed_sha = Some("base_sha".to_string());
        index.branch.meta.indexed_sha = Some("branch_sha".to_string());

        // Save the index
        cache.save_layered_index(&index).expect("Failed to save layered index");

        // Verify meta file exists
        assert!(cache.layer_meta_path().exists(), "Meta file should exist");

        // Verify has_cached_layers returns true
        assert!(cache.has_cached_layers(), "Should detect cached layers");

        // Load the index back
        let loaded = cache.load_layered_index().expect("Failed to load layered index");
        assert!(loaded.is_some(), "Should load the saved index");

        let loaded_index = loaded.unwrap();
        assert_eq!(loaded_index.base.meta.indexed_sha, Some("base_sha".to_string()));
        assert_eq!(loaded_index.branch.meta.indexed_sha, Some("branch_sha".to_string()));
        // AI layer should be fresh (empty)
        assert!(loaded_index.ai.meta.indexed_sha.is_none());
    }

    #[test]
    fn test_clear_layers() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheDir {
            root: temp_dir.path().to_path_buf(),
            repo_root: temp_dir.path().to_path_buf(),
            repo_hash: "test_hash".to_string(),
        };

        // Save some layers
        let index = LayeredIndex::new();
        cache.save_layered_index(&index).expect("Failed to save");
        assert!(cache.has_cached_layers());

        // Clear the layers
        cache.clear_layers().expect("Failed to clear layers");

        // Verify layers are gone
        assert!(!cache.has_cached_layers(), "Layers should be cleared");
        assert!(!cache.layers_dir().exists(), "Layers directory should be removed");
    }

    #[test]
    fn test_load_missing_layer() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let cache = CacheDir {
            root: temp_dir.path().to_path_buf(),
            repo_root: temp_dir.path().to_path_buf(),
            repo_hash: "test_hash".to_string(),
        };

        // Loading a layer that doesn't exist should return None
        let result = cache.load_layer(LayerKind::Base).expect("Load should not error");
        assert!(result.is_none(), "Should return None for missing layer");
    }

    #[test]
    fn test_layered_index_meta_serialization() {
        let meta = LayeredIndexMeta {
            schema_version: "1.0.0".to_string(),
            saved_at: "2024-01-01T00:00:00Z".to_string(),
            base_indexed_sha: Some("abc123".to_string()),
            branch_indexed_sha: None,
            merge_base: Some("def456".to_string()),
        };

        // Serialize and deserialize
        let json = serde_json::to_string(&meta).expect("Serialize failed");
        let restored: LayeredIndexMeta = serde_json::from_str(&json).expect("Deserialize failed");

        assert_eq!(restored.schema_version, "1.0.0");
        assert_eq!(restored.base_indexed_sha, Some("abc123".to_string()));
        assert_eq!(restored.branch_indexed_sha, None);
        assert_eq!(restored.merge_base, Some("def456".to_string()));
    }
}
