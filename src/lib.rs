//! fhtml — compiler for Fluid HTML, a token-cheap, Tailwind-native markup
//! language. See SPEC.md for the normative language definition.
//!
//! v0.1 implements the static markup layer (SPEC §1–§8, §11) plus the canonical
//! formatter. Template-layer constructs (§9–§10) are recognized and rejected
//! with a clear error.

#[cfg(feature = "convert")]
pub mod convert;
mod emit;
mod error;
mod fmt;
mod parser;

pub use emit::Mode;
pub use error::Error;

/// A successful compile: the HTML plus non-fatal warnings (e.g. suspicious
/// indent steps, SPEC §2). Warning strings are `line:col: warning: …`.
pub struct Output {
    pub html: String,
    pub warnings: Vec<String>,
}

/// Compiles fhtml source to HTML, discarding warnings.
pub fn compile(src: &str, mode: Mode) -> Result<String, Error> {
    Ok(compile_full(src, mode)?.html)
}

/// Compiles fhtml source to HTML, returning warnings alongside.
pub fn compile_full(src: &str, mode: Mode) -> Result<Output, Error> {
    let (nodes, warnings) = parser::parse(src)?;
    Ok(Output {
        html: emit::emit(&nodes, mode),
        warnings,
    })
}

/// Reformats fhtml source into canonical form: 2-space indentation (spaces
/// only), `.` for `div`, minimal quoting. `compile(format(s)) == compile(s)`.
pub fn format(src: &str) -> Result<String, Error> {
    let (nodes, _) = parser::parse(src)?;
    Ok(fmt::format_nodes(&nodes))
}
