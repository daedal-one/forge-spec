//! Reusable implementation of the Specs Format toolchain.
//!
//! The `spec` binary is intentionally a thin adapter over this library so
//! desktop applications and language-server integrations share one parser,
//! registry, linter, and renderer.

pub mod cli;
pub mod commands;
pub mod graph;
pub mod history;
pub mod lint;
pub mod lsp;
pub mod model;
pub mod parse;
pub mod render;
pub mod symbol;
