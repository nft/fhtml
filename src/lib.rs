//! fhtml — compiler for Fluid HTML, a token-cheap, Tailwind-native markup
//! language. See SPEC.md for the normative language definition.
//!
//! v0.1 implements the static markup layer (SPEC §1–§8, §11). Template-layer
//! constructs (§9–§10) are recognized and rejected with a clear error.

mod emit;
mod error;
mod parser;

pub use emit::Mode;
pub use error::Error;

/// Compiles fhtml source to HTML.
pub fn compile(src: &str, mode: Mode) -> Result<String, Error> {
    let nodes = parser::parse(src)?;
    Ok(emit::emit(&nodes, mode))
}
