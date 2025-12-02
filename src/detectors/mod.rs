//! Language-specific semantic detectors
//!
//! This module contains specialized extractors for different language families.
//! Each detector knows how to extract semantic information from AST nodes
//! for its target language(s).
//!
//! # Architecture
//!
//! Detectors are organized by language family:
//! - `javascript`: JS, TS, JSX, TSX
//! - `rust`: Rust
//! - `python`: Python
//! - `go`: Go
//! - `java`: Java
//! - `c_family`: C, C++
//! - `markup`: HTML, CSS, Markdown
//! - `config`: JSON, YAML, TOML
//!
//! Each detector module implements extraction functions that populate
//! a `SemanticSummary` struct with language-specific information.
//!
//! # Symbol Selection Heuristics (Priority 3.0F)
//!
//! All detectors implement improved symbol selection:
//! - Prioritize public/exported symbols over private helpers
//! - Prefer types (structs/classes/enums) over functions where applicable
//! - Consider filename matching (e.g., `toon.rs` → prefer `Toon` or `encode_toon`)
//! - For multi-symbol files, select the most semantically significant symbol

pub mod c_family;
pub mod common;
pub mod config;
pub mod go;
pub mod java;
pub mod javascript;
pub mod markup;
pub mod python;
pub mod rust;
