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
//! Each detector module will implement extraction functions that populate
//! a `SemanticSummary` struct with language-specific information.

// Detector modules will be added here as they are implemented
// pub mod javascript;
// pub mod rust;
// pub mod python;
// pub mod go;
// pub mod java;
// pub mod c_family;
// pub mod markup;
// pub mod config;
