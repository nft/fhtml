//! fhtml — compiler for Fluid HTML, a token-cheap, Tailwind-native markup
//! language. See SPEC.md for the normative language definition.
//!
//! Implements the static markup layer (SPEC §1–§8, §11), the canonical formatter,
//! the template layer (SPEC §9 interpolation, §10.1–§10.2 statements),
//! and components (§10.3–§10.4 `def`/`+call`/`children`) on the render and
//! `fmt` paths. The JS backend for components lands later. `include` (§10.5) is recognized and rejected
//! with a clear "not implemented" error.

#[cfg(feature = "convert")]
pub mod convert;
mod emit;
mod error;
pub mod expr;
mod fmt;
mod jsgen;
pub mod json;
mod parser;
pub mod shorthand;

pub use emit::Mode;
pub use error::Error;
pub use expr::Value;

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
    let (doc, warnings) = parser::parse(src, opts.templates)?;
    if let Some((line, col, what)) = parser::first_template_use_doc(&doc) {
        return error::err(
            line,
            col,
            format!("{what} is a template construct — static compilation cannot render it; pass data (`--data`, or the `render` API)"),
        );
    }
    // A literal-only tree evaluates nothing, so this cannot error.
    Ok(Output {
        html: emit::render_document(&doc, opts.mode, &Value::Null, &Value::Null)?,
        warnings,
    })
}

/// Renders fhtml source against `data` (SPEC §9–§10), with a null `ctx` and
/// no warnings. Template-free files render identically to [`compile`]; a
/// null/absent value for any name simply resolves to `null`.
pub fn render(src: &str, data: &Value, mode: Mode) -> Result<String, Error> {
    Ok(render_full(src, data, &Value::Null, mode)?.html)
}

/// Renders with an explicit `ctx` — the read-only, host-provided context map
/// bound to the reserved root name `ctx` in every scope (SPEC §9.4) — and
/// returns warnings alongside. Render errors carry the file line/column of
/// the offending interpolation or statement, like parse errors.
pub fn render_full(src: &str, data: &Value, ctx: &Value, mode: Mode) -> Result<Output, Error> {
    let (doc, warnings) = parser::parse(src, true)?;
    Ok(Output {
        html: emit::render_document(&doc, mode, data, ctx)?,
        warnings,
    })
}

/// Compiles fhtml source to a self-contained ES module exporting
/// `(data, ctx = {}) => string` with semantics identical to [`render`]
/// (SPEC §11 `--target=js`). Static files compile to a constant function,
/// for uniformity. The returned [`Output`]'s `html` field holds the module
/// source text.
pub fn compile_to_js(src: &str, mode: Mode) -> Result<Output, Error> {
    let (doc, warnings) = parser::parse(src, true)?;
    // Gate: the JS backend for
    // components is not implemented yet.
    if let Some((line, what)) = parser::first_p2_use(&doc) {
        return error::err(
            line,
            1,
            format!("{what} has no `--target=js` support yet — components parse, but the JS backend for them is not implemented"),
        );
    }
    Ok(Output {
        html: jsgen::generate(&doc.body, mode),
        warnings,
    })
}

/// Reformats fhtml source into canonical form: 2-space indentation (spaces
/// only), `.` for `div`, minimal quoting. Template files format too —
/// expressions are reprinted from source text. Invariants:
/// `compile(format(s)) == compile(s)` and `format(format(s)) == format(s)`.
pub fn format(src: &str) -> Result<String, Error> {
    let (doc, _) = parser::parse(src, true)?;
    Ok(fmt::format_document(&doc))
}
