//! fhtml — compiler for Fluid HTML, a token-cheap, Tailwind-native markup
//! language. See SPEC.md for the normative language definition.
//!
//! Implements the static markup layer (SPEC §1–§8, §11), the canonical formatter,
//! the template layer (SPEC §9 interpolation, §10.1–§10.2 statements),
//! and the composition layer (§10.3–§10.5 `def`/`+call`/`children` and
//! `include`) across the whole toolchain — render, `fmt`, and `--target=js`.
//! Includes need a file context: use the `_from` entry points (or the CLI,
//! which passes the source path); the string-only entry points reject
//! `include` since stdin has no base path.

#[cfg(feature = "convert")]
pub mod convert;
mod emit;
mod error;
pub mod expr;
mod fmt;
mod jsgen;
pub mod json;
mod parser;
mod resolve;
pub mod shorthand;

pub use emit::Mode;
pub use error::Error;
pub use expr::Value;

/// Whether bare class tokens decode through the shorthand codebook
/// (SPEC §3.2, [`shorthand`]). `Auto` lets each file's `#!shorthand`
/// directive decide; `On`/`Off` force it for every file in the compilation,
/// includes included. `Off` is *lexical*-off — the file parses as if no
/// directive were present, so the `=` escape is inert too (`=ti4` stays the
/// literal class `=ti4`). Directive placement is validated under every
/// policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ShorthandPolicy {
    #[default]
    Auto,
    On,
    Off,
}

/// How [`format_shorthand`] treats shorthand class tokens (SPEC §3.2). Both
/// rewrites preserve `compile(format(s)) == compile(s)`: they change how each
/// class is *written*, never what it means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FmtShorthand {
    /// Reprint the authored form: codes stay codes, verbatim stays verbatim,
    /// the directive line survives. What [`format`] does.
    #[default]
    Preserve,
    /// Decode every code to its full class, resolve `=` escapes, and drop the
    /// `#!shorthand` directive — the file leaves shorthand form entirely.
    Expand,
    /// Contract every class to its code where one round-trips, `=`-escape
    /// classes that would decode as something else, and open the file with
    /// `#!shorthand`.
    Contract,
}

/// Compile options beyond the output [`Mode`].
#[derive(Debug)]
pub struct Options {
    pub mode: Mode,
    /// `false` enforces static-only (SPEC §9.2): any template construct — statements,
    /// `{…}` interpolation, unescaped `{` in text — is a parse error.
    pub templates: bool,
    /// Class-shorthand decoding (SPEC §3.2); `Auto` honors each file's
    /// `#!shorthand` directive.
    pub shorthand: ShorthandPolicy,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Min,
            templates: true,
            shorthand: ShorthandPolicy::Auto,
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
            ..Options::default()
        },
    )
}

/// Compiles with explicit [`Options`]. Template constructs are always an
/// error on this static path; `templates: false` additionally rejects them
/// at parse time with static-path wording (SPEC §9.2) and requires `\{` for literal
/// braces in text.
pub fn compile_opts(src: &str, opts: &Options) -> Result<Output, Error> {
    let (doc, warnings) = parser::parse(src, opts.templates, opts.shorthand)?;
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
///
/// No file context: a source using `include` (SPEC §10.5) is an error here —
/// use [`render_full_from`] with the source's path.
pub fn render_full(src: &str, data: &Value, ctx: &Value, mode: Mode) -> Result<Output, Error> {
    render_full_from(src, None, data, ctx, mode)
}

/// [`render_full`] with the path the source was read from, which makes
/// `include` (SPEC §10.5) resolvable: paths are relative to `file`, `.fhtml`
/// is appended if absent, included `def`s join the document's namespace, and
/// include cycles are errors listing the chain. `None` behaves exactly like
/// [`render_full`].
pub fn render_full_from(
    src: &str,
    file: Option<&std::path::Path>,
    data: &Value,
    ctx: &Value,
    mode: Mode,
) -> Result<Output, Error> {
    render_opts_from(
        src,
        file,
        data,
        ctx,
        &Options {
            mode,
            ..Options::default()
        },
    )
}

/// [`render_full_from`] with explicit [`Options`] — the render path takes
/// `opts.mode` and `opts.shorthand` from here (the policy reaches included
/// files too, SPEC §3.2). `opts.templates` is ignored: rendering *is* the
/// template path; use [`compile_opts`] for static-only enforcement.
pub fn render_opts_from(
    src: &str,
    file: Option<&std::path::Path>,
    data: &Value,
    ctx: &Value,
    opts: &Options,
) -> Result<Output, Error> {
    let (doc, mut warnings) = parser::parse(src, true, opts.shorthand)?;
    let doc = resolve::resolve_includes(doc, file, opts.shorthand, &mut warnings)?;
    Ok(Output {
        html: emit::render_document(&doc, opts.mode, data, ctx)?,
        warnings,
    })
}

/// Compiles fhtml source to a self-contained ES module exporting
/// `(data, ctx = {}) => string` with semantics identical to [`render`]
/// (SPEC §11 `--target=js`). Static files compile to a constant function,
/// for uniformity. The returned [`Output`]'s `html` field holds the module
/// source text. Like [`render_full`], sources using `include` need the
/// `_from` variant.
pub fn compile_to_js(src: &str, mode: Mode) -> Result<Output, Error> {
    compile_to_js_from(src, None, mode)
}

/// [`compile_to_js`] with the source's path: includes are inlined, so the
/// emitted module stays self-contained — one module out regardless of how
/// many files went in (SPEC §10.5).
pub fn compile_to_js_from(
    src: &str,
    file: Option<&std::path::Path>,
    mode: Mode,
) -> Result<Output, Error> {
    compile_to_js_opts_from(
        src,
        file,
        &Options {
            mode,
            ..Options::default()
        },
    )
}

/// [`compile_to_js_from`] with explicit [`Options`] — `opts.shorthand`
/// reaches included files too (SPEC §3.2). `opts.templates` is ignored: the
/// emitted module is the template path by construction.
pub fn compile_to_js_opts_from(
    src: &str,
    file: Option<&std::path::Path>,
    opts: &Options,
) -> Result<Output, Error> {
    let (doc, mut warnings) = parser::parse(src, true, opts.shorthand)?;
    let doc = resolve::resolve_includes(doc, file, opts.shorthand, &mut warnings)?;
    Ok(Output {
        html: jsgen::generate(&doc, opts.mode)?,
        warnings,
    })
}

/// Reformats fhtml source into canonical form: 2-space indentation (spaces
/// only), `.` for `div`, minimal quoting. Template files format too —
/// expressions are reprinted from source text. Invariants:
/// `compile(format(s)) == compile(s)` and `format(format(s)) == format(s)`.
pub fn format(src: &str) -> Result<String, Error> {
    format_shorthand(src, FmtShorthand::Preserve)
}

/// [`format`] with an explicit shorthand mode: [`FmtShorthand::Expand`] and
/// [`FmtShorthand::Contract`] rewrite between the verbatim and shorthand
/// forms of the class list (SPEC §3.2) — output-preserving in both
/// directions, on files with or without the directive.
pub fn format_shorthand(src: &str, shorthand: FmtShorthand) -> Result<String, Error> {
    // `Off` preserves the authored form: no decode, `=` escapes untouched
    // (lexical-off), the `#!shorthand` directive recorded on the Document and
    // re-emitted by the formatter. fmt never emits HTML, so it never needs
    // the decoded classes (SPEC §3.2) — Expand/Contract rewrite the authored
    // tokens themselves at print time.
    let (doc, _) = parser::parse(src, true, ShorthandPolicy::Off)?;
    Ok(fmt::format_document(&doc, shorthand))
}
