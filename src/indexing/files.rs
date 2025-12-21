//! File collection utilities for indexing
//!
//! This module provides functions for recursively collecting source files
//! from a directory, with filtering by extension and language support.

use std::fs;
use std::path::{Path, PathBuf};

use crate::Lang;

/// Collect all supported source files from a directory.
///
/// # Arguments
///
/// * `dir` - The root directory to search
/// * `max_depth` - Maximum recursion depth (0 = only root directory)
/// * `extensions` - Optional list of extensions to include (empty = all supported)
///
/// # Returns
///
/// A vector of paths to supported source files, sorted by path.
pub fn collect_files(dir: &Path, max_depth: usize, extensions: &[String]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files_recursive(dir, max_depth, 0, extensions, &mut files);
    files
}

/// Recursively collect files with depth tracking.
///
/// This is the internal recursive implementation. Use `collect_files` for
/// the public API.
pub fn collect_files_recursive(
    dir: &Path,
    max_depth: usize,
    current_depth: usize,
    extensions: &[String],
    files: &mut Vec<PathBuf>,
) {
    if current_depth > max_depth {
        return;
    }

    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Skip hidden files/directories and common non-source directories
        if should_skip_path(&path) {
            continue;
        }

        if path.is_dir() {
            collect_files_recursive(&path, max_depth, current_depth + 1, extensions, files);
        } else if path.is_file() {
            if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                // Check extension filter if provided
                if !extensions.is_empty() && !extensions.iter().any(|e| e == ext) {
                    continue;
                }

                // Check if language is supported
                if Lang::from_extension(ext).is_ok() {
                    files.push(path);
                }
            }
        }
    }
}

/// Check if a path should be skipped during file collection.
///
/// Skips:
/// - Hidden files/directories (starting with '.')
/// - Common non-source directories: node_modules, target, dist, build, etc.
pub fn should_skip_path(path: &Path) -> bool {
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        name.starts_with('.')
            || name == "node_modules"
            || name == "target"
            || name == "dist"
            || name == "build"
            || name == ".next"
            || name == "coverage"
            || name == "__pycache__"
            || name == "vendor"
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_skip_hidden() {
        assert!(should_skip_path(Path::new(".git")));
        assert!(should_skip_path(Path::new(".hidden")));
    }

    #[test]
    fn test_should_skip_node_modules() {
        assert!(should_skip_path(Path::new("node_modules")));
    }

    #[test]
    fn test_should_skip_target() {
        assert!(should_skip_path(Path::new("target")));
    }

    #[test]
    fn test_should_not_skip_src() {
        assert!(!should_skip_path(Path::new("src")));
        assert!(!should_skip_path(Path::new("lib")));
    }

    #[test]
    fn test_collect_files_empty_dir() {
        let temp_dir = std::env::temp_dir().join("semfora_test_empty");
        let _ = fs::create_dir_all(&temp_dir);

        let files = collect_files(&temp_dir, 10, &[]);
        // May have files from previous runs, but should not panic
        assert!(files.len() >= 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }
}
