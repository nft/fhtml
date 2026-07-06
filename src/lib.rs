//! fhtml — compiler for Fluid HTML, a token-cheap, Tailwind-native markup
//! language. See SPEC.md for the normative language definition.
//!
//! Implements the static markup layer (SPEC §1–§8, §11), the canonical formatter,
//! and the template layer (SPEC §9 interpolation, §10.1–§10.2 statements).
//! Composition constructs (§10.3–§10.5) are recognized and rejected with a
//! clear "composition-layer" error.

#[cfg(feature = "convert")]
pub mod convert;
mod emit;
mod error;
pub mod expr;
mod fmt;
pub mod json;
mod parser;

pub use emit::Mode;
pub use error::Error;

/// Compile options beyond the output [`Mode`].
#[derive(Debug)]
pub struct Options {
    pub mode: Mode,
    /// `false` enforces static-only (SPEC §9.2): any template construct — statements,
    /// `{…}` interpolation, unescaped `{` in text — is a parse error.
    pub templates: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Min,
            templates: true,
        }
    }
}

/// A successful compile: the HTML plus non-fatal warnings (e.g. suspicious
/// indent steps, SPEC §2). Warning strings are `line:col: warning: …`.
#[derive(Debug)]
pub struct Output {
    pub html: String,
    pub warnings: Vec<String>,
}

/// Compiles fhtml source to HTML, discarding warnings.
///
/// This is the static path: a file that uses the template layer is an error
/// here — render it with data instead.
pub fn compile(src: &str, mode: Mode) -> Result<String, Error> {
    Ok(compile_full(src, mode)?.html)
}

/// Compiles fhtml source to HTML, returning warnings alongside.
pub fn compile_full(src: &str, mode: Mode) -> Result<Output, Error> {
    compile_opts(
        src,
        &Options {
            mode,
            templates: true,
        },
    )
}

/// Compiles with explicit [`Options`]. Template constructs are always an
/// error on this static path; `templates: false` additionally rejects them
/// at parse time with static-path wording (SPEC §9.2) and requires `\{` for literal
/// braces in text.
pub fn compile_opts(src: &str, opts: &Options) -> Result<Output, Error> {
    let (nodes, warnings) = parser::parse(src, opts.templates)?;
    if let Some((line, col, what)) = parser::first_template_use(&nodes) {
        return error::err(
            line,
            col,
            format!("{what} is a template construct — static compilation cannot render it; pass data (`--data`, or the `render` API)"),
        );
    }
    Ok(Output {
        html: emit::emit(&nodes, opts.mode),
        warnings,
    })
}

/// Reformats fhtml source into canonical form: 2-space indentation (spaces
/// only), `.` for `div`, minimal quoting. Template files format too —
/// expressions are reprinted from source text. Invariants:
/// `compile(format(s)) == compile(s)` and `format(format(s)) == format(s)`.
pub fn format(src: &str) -> Result<String, Error> {
    let (nodes, _) = parser::parse(src, true)?;
    Ok(fmt::format_nodes(&nodes))
}
