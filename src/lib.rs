//! ai-nsc — the compiler for **NovaSharp**, a statically typed language targeting
//! native code through LLVM 18.
//!
//! The crate is organised as a pipeline:
//!
//! - [`source`] — file buffers, source locations and the line/column mapping
//! - [`diagnostics`] — severities, counts and the `file:line:col` renderer
//! - [`driver`] — command line parsing and action dispatch
//! - [`lexer`] — token kinds and the hand-written lexer
//! - [`codegen`] — LLVM IR generation (the only module that touches LLVM)
//!
//! The lexer, parser and semantic analysis live in this crate too; they are
//! added once the NovaSharp grammar specification is available.

#![deny(unsafe_code)]

pub mod ast;
mod ast_clone;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod lexer;
pub mod literal;
pub mod parser;
pub mod sema;
pub mod source;

pub use diagnostics::{DiagnosticEngine, Severity};
pub use source::{FileId, SourceLocation, SourceManager, Span};

/// Version of the compiler itself.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Version of the LLVM installation the compiler was linked against.
pub const LLVM_VERSION: &str = env!("AINSC_LLVM_VERSION");

/// LLVM's idea of the host target triple.
pub const HOST_TRIPLE: &str = env!("AINSC_HOST_TRIPLE");
