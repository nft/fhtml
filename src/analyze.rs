//! Source analysis for tooling (the LSP, editors): positions and spans for
//! every `def`, `+call`, and `include` in a file, plus structured diagnostics
//!.
//!
//! The parser's AST stays crate-private; [`analyze`] is the public,
//! purpose-built surface. It never fails: an unparsable file still yields the
//! error as a [`Diag`] and a best-effort symbol list from a line-prefix
//! rescan (no parser recovery mode — SPEC §11 keeps first-error semantics).
//!
//! Positions follow the compiler's convention (SPEC §11): 1-based lines,
//! 1-based columns counting *characters* from the physical line start,
//! indentation included. Span lengths are in characters too. LSP UTF-16
//! conversion is the transport's job, not this module's.

use std::path::{Path, PathBuf};

use crate::error::Error;
use crate::parser::{self, Def, Document, Node};
use crate::vfs::{DiskVfs, Vfs};
use crate::{resolve, ShorthandPolicy};

/// A source span: 1-based `line` and `col`, `len` characters long, all
/// counted in chars on the physical line (SPEC §11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub len: usize,
}

/// One diagnostic — the parse/resolve error or a compiler warning. `len`
/// runs from `col` to the end of the source line (at least 1): point
/// positions widened so a consumer can underline something.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diag {
    pub line: usize,
    pub col: usize,
    pub len: usize,
    pub msg: String,
}

/// One `def name(params)` component definition (SPEC §10.3).
#[derive(Debug, Clone)]
pub struct DefSym {
    pub name: String,
    /// The name token on the `def` line.
    pub name_span: Span,
    pub params: Vec<ParamSym>,
    /// Last line of the definition, body included (the `def` line itself
    /// for an empty body).
    pub end_line: usize,
    /// The file the definition lives in — `None` for the analyzed source
    /// itself, `Some` (canonical path) for a def reached through `include`.
    /// Spans of a `Some` def are positions in *that* file.
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ParamSym {
    pub name: String,
    /// The parameter name token inside the `def` line's parens. Falls back
    /// to the def's name span when the token can't be located on the
    /// physical line (`\`-continued parameter lists, SPEC §11).
    pub name_span: Span,
    /// Default value's source text, if the parameter has one.
    pub default: Option<String>,
}

/// One `+name(args)` component call (SPEC §10.4), anywhere in the analyzed
/// source — top level, nested, or inside a `def` body.
#[derive(Debug, Clone)]
pub struct CallSym {
    pub name: String,
    /// The name token after the `+`.
    pub name_span: Span,
    pub args: Vec<ArgSym>,
}

#[derive(Debug, Clone)]
pub struct ArgSym {
    pub name: String,
    /// The argument name token inside the call's parens.
    pub span: Span,
}

/// One `include <path>` line (SPEC §10.5).
#[derive(Debug, Clone)]
pub struct IncludeSym {
    /// The path exactly as written.
    pub path: String,
    /// The path text on the `include` line.
    pub span: Span,
    /// The canonical path of the target file, when the analyzed source has
    /// a file path and the target exists on disk.
    pub resolved: Option<PathBuf>,
}

/// Everything tooling needs from one source: symbols with spans, structured
/// warnings, and the error if the source doesn't compile.
#[derive(Debug)]
pub struct Analysis {
    /// Own-file definitions first (in source order), then definitions from
    /// included files (`file: Some`) when a path was given.
    pub defs: Vec<DefSym>,
    pub calls: Vec<CallSym>,
    /// Own-file `include` lines only (top level, like the language allows).
    pub includes: Vec<IncludeSym>,
    pub warnings: Vec<Diag>,
    /// The compile error, if any: parse errors of this source, and
    /// include-resolution errors (missing file, cycle, `def` collision —
    /// or, without a file path, the includes-need-a-base-path error),
    /// positioned exactly as `compile`/`render` report them.
    pub error: Option<Diag>,
}

/// Analyzes fhtml source for tooling. Never fails: a source that doesn't
/// parse still returns [`Analysis::error`] plus a best-effort symbol list
/// (line-prefix rescan; no parser recovery). With `file` given, includes
/// resolve (SPEC §10.5) so definitions from included files appear in
/// [`Analysis::defs`] with their own file and positions; without it
/// (unsaved buffer, stdin), symbols are same-file only.
pub fn analyze(src: &str, file: Option<&Path>) -> Analysis {
    analyze_vfs(src, file, &DiskVfs)
}

/// [`analyze`] with an explicit file loader: include resolution and the
/// cross-file definition sweep go through `vfs` — a [`crate::MemVfs`] file
/// map gives browser editors the same cross-file diagnostics and
/// definitions the LSP gets from disk.
pub fn analyze_vfs(src: &str, file: Option<&Path>, vfs: &dyn Vfs) -> Analysis {
    let lines = split_lines(src);
    match parser::parse(src, true, ShorthandPolicy::Auto) {
        Ok((doc, warnings)) => {
            let mut a = Analysis {
                defs: doc.defs.iter().map(|d| def_sym(d, &lines, None)).collect(),
                calls: collect_calls(&doc, &lines),
                includes: doc
                    .body
                    .iter()
                    .filter_map(|n| match n {
                        Node::Include { path, line } => Some(IncludeSym {
                            path: path.clone(),
                            span: include_path_span(&lines, *line, path),
                            resolved: None,
                        }),
                        _ => None,
                    })
                    .collect(),
                warnings: warnings.iter().map(|w| warning_diag(w, &lines)).collect(),
                error: None,
            };
            if let Some(file) = file {
                for inc in &mut a.includes {
                    inc.resolved = vfs.locate(&resolve::include_target(file, &inc.path));
                }
            }
            // The real resolution pass, for exact compile-error parity
            // (missing file, cycle, def collision — and, with no file
            // path, the includes-need-a-base-path error) and the
            // transitive dep list in first-include order.
            let mut sink = Vec::new();
            let mut deps = Vec::new();
            if let Err(e) = resolve::resolve_includes(
                doc,
                file,
                ShorthandPolicy::Auto,
                &mut sink,
                &mut deps,
                vfs,
            ) {
                a.error = Some(error_diag(&e, &lines));
            }
            for dep in deps {
                a.defs.extend(file_defs(&dep, vfs));
            }
            a
        }
        Err(e) => {
            let mut a = rescan(&lines, file, vfs);
            a.error = Some(error_diag(&e, &lines));
            a
        }
    }
}

// ---- span location --------------------------------------------------------
// The AST records lines but not name columns; symbol lines have a fixed,
// parser-validated shape (`def name(…)`, `+name(…)`, `include path`), so the
// column is recovered from the source line itself. The fallback — a span
// over the line's trimmed content — only triggers for `\`-continued lines,
// where columns past the first physical line have no exact mapping anyway.

fn split_lines(src: &str) -> Vec<&str> {
    src.split('\n')
        .map(|l| l.strip_suffix('\r').unwrap_or(l))
        .collect()
}

/// Char index of the first non-indent char, or `None` for a blank line.
fn indent_width(text: &str) -> Option<usize> {
    text.chars().position(|c| c != ' ' && c != '\t')
}

fn line_span_fallback(lines: &[&str], line: usize) -> Span {
    let text = lines.get(line - 1).copied().unwrap_or("");
    let col = indent_width(text).unwrap_or(0) + 1;
    Span {
        line,
        col,
        len: text.trim().chars().count().max(1),
    }
}

/// Span of `name` where the line reads `<indent>def name…` / `<indent>+name…`
/// (`prefix` = the chars between indent and name).
fn name_span(lines: &[&str], line: usize, prefix: &str, name: &str) -> Span {
    let Some(text) = lines.get(line - 1) else {
        return line_span_fallback(lines, line);
    };
    let chars: Vec<char> = text.chars().collect();
    let mut i = indent_width(text).unwrap_or(0);
    for p in prefix.chars() {
        if chars.get(i) == Some(&p) {
            i += 1;
        } else {
            return line_span_fallback(lines, line);
        }
    }
    while matches!(chars.get(i), Some(' ' | '\t')) {
        i += 1;
    }
    let name_chars: Vec<char> = name.chars().collect();
    if chars.get(i..i + name_chars.len()) == Some(&name_chars[..]) {
        Span {
            line,
            col: i + 1,
            len: name_chars.len(),
        }
    } else {
        line_span_fallback(lines, line)
    }
}

fn include_path_span(lines: &[&str], line: usize, path: &str) -> Span {
    name_span(lines, line, "include", path)
}

/// Last line of a top-level block opened at `start`: every following
/// non-blank line that is indented belongs to it (defs and includes are top
/// level, so "indented" is exact — SPEC §10.3).
fn block_end_line(lines: &[&str], start: usize) -> usize {
    let mut end = start;
    for (idx, text) in lines.iter().enumerate().skip(start) {
        match indent_width(text) {
            None => continue,
            Some(0) => break,
            Some(_) => end = idx + 1,
        }
    }
    end
}

fn def_sym(d: &Def, lines: &[&str], file: Option<PathBuf>) -> DefSym {
    let name_span = name_span(lines, d.line, "def", &d.name);
    let raw: Vec<(String, Option<String>)> = d
        .params
        .iter()
        .map(|p| (p.name.clone(), p.default.as_ref().map(|t| t.src.clone())))
        .collect();
    let params = param_syms(raw, lines, &name_span);
    DefSym {
        name: d.name.clone(),
        name_span,
        params,
        end_line: block_end_line(lines, d.line),
        file,
    }
}

/// Builds [`ParamSym`]s, locating each name token inside the `def` line's
/// parens: walk the list after the def name with the same quote/brace
/// awareness as [`rescan_params`] and match top-level tokens to the expected
/// names in order. A name that can't be matched on the physical line (a
/// `\`-continued list) keeps the def-name fallback span.
fn param_syms(
    raw: Vec<(String, Option<String>)>,
    lines: &[&str],
    def_span: &Span,
) -> Vec<ParamSym> {
    let mut params: Vec<ParamSym> = raw
        .into_iter()
        .map(|(name, default)| ParamSym {
            name,
            name_span: def_span.clone(),
            default,
        })
        .collect();
    let text = lines.get(def_span.line - 1).copied().unwrap_or("");
    let chars: Vec<char> = text.chars().collect();
    let mut i = def_span.col - 1 + def_span.len;
    while matches!(chars.get(i), Some(' ' | '\t')) {
        i += 1;
    }
    if chars.get(i) != Some(&'(') {
        return params;
    }
    i += 1;
    // Top-level token starts/ends (char indices) inside the paren list.
    let mut tokens: Vec<(usize, usize)> = Vec::new();
    let mut start: Option<usize> = None;
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    start.get_or_insert(i);
                    quote = Some(c);
                }
                '{' => {
                    start.get_or_insert(i);
                    depth += 1;
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                }
                ')' if depth == 0 => {
                    if let Some(s) = start.take() {
                        tokens.push((s, i));
                    }
                    break;
                }
                ' ' | '\t' if depth == 0 => {
                    if let Some(s) = start.take() {
                        tokens.push((s, i));
                    }
                }
                _ => {
                    start.get_or_insert(i);
                }
            },
        }
        i += 1;
    }
    if let Some(s) = start {
        tokens.push((s, chars.len()));
    }
    let mut ti = 0;
    for p in &mut params {
        while ti < tokens.len() {
            let (s, e) = tokens[ti];
            ti += 1;
            let tok: String = chars[s..e].iter().collect();
            if ident_prefix(&tok) == p.name {
                p.name_span = Span {
                    line: def_span.line,
                    col: s + 1,
                    len: p.name.chars().count(),
                };
                break;
            }
        }
    }
    params
}

fn collect_calls(doc: &Document, lines: &[&str]) -> Vec<CallSym> {
    let mut out = Vec::new();
    for d in &doc.defs {
        walk_calls(&d.body, lines, &mut out);
    }
    walk_calls(&doc.body, lines, &mut out);
    out.sort_by_key(|c| (c.name_span.line, c.name_span.col));
    out
}

fn walk_calls(nodes: &[Node], lines: &[&str], out: &mut Vec<CallSym>) {
    for n in nodes {
        match n {
            Node::Call(c) => {
                out.push(CallSym {
                    name: c.name.clone(),
                    name_span: name_span(lines, c.line, "+", &c.name),
                    args: c
                        .args
                        .iter()
                        .map(|a| ArgSym {
                            name: a.name.clone(),
                            span: Span {
                                line: a.line,
                                col: a.col,
                                len: a.name.chars().count(),
                            },
                        })
                        .collect(),
                });
                walk_calls(&c.children, lines, out);
            }
            Node::Element(el) => {
                let mut el = el;
                loop {
                    walk_calls(&el.children, lines, out);
                    match &el.chain {
                        Some(next) => el = next,
                        None => break,
                    }
                }
            }
            Node::If(chain) => {
                for arm in &chain.arms {
                    walk_calls(&arm.body, lines, out);
                }
                if let Some(body) = &chain.else_body {
                    walk_calls(body, lines, out);
                }
            }
            Node::For(f) => {
                walk_calls(&f.body, lines, out);
                if let Some(empty) = &f.empty {
                    walk_calls(empty, lines, out);
                }
            }
            Node::TextBlock(_)
            | Node::Raw(_)
            | Node::Comment { .. }
            | Node::Doctype
            | Node::Children { .. }
            | Node::DefSite(_)
            | Node::Include { .. } => {}
        }
    }
}

/// Definitions of one included file, tagged with its canonical path. A file
/// that can't be read or parsed contributes nothing — the resolution pass
/// already reported that as [`Analysis::error`].
fn file_defs(path: &Path, vfs: &dyn Vfs) -> Vec<DefSym> {
    let Ok(src) = vfs.read(path) else {
        return Vec::new();
    };
    let Ok((doc, _)) = parser::parse(&src, true, ShorthandPolicy::Auto) else {
        return Vec::new();
    };
    let lines = split_lines(&src);
    doc.defs
        .iter()
        .map(|d| def_sym(d, &lines, Some(path.to_path_buf())))
        .collect()
}

// ---- diagnostics ----------------------------------------------------------

/// Widens a point position to a span reaching the end of its source line —
/// enough for an underline; never zero-length.
fn point_diag(line: usize, col: usize, msg: String, lines: &[&str]) -> Diag {
    let total = lines.get(line - 1).map_or(0, |t| t.chars().count());
    Diag {
        line,
        col,
        len: (total + 1).saturating_sub(col).max(1),
        msg,
    }
}

fn error_diag(e: &Error, lines: &[&str]) -> Diag {
    point_diag(e.line, e.col, e.msg.clone(), lines)
}

/// Parses the compiler's own `line:col: warning: msg` format (the one
/// [`crate::Output`] documents) back into a structured diagnostic.
fn warning_diag(w: &str, lines: &[&str]) -> Diag {
    let parse = || {
        let (line, rest) = w.split_once(':')?;
        let (col, rest) = rest.split_once(':')?;
        let msg = rest.trim_start().strip_prefix("warning:")?.trim_start();
        Some((line.parse().ok()?, col.parse().ok()?, msg.to_string()))
    };
    match parse() {
        Some((line, col, msg)) => point_diag(line, col, msg, lines),
        None => point_diag(1, 1, w.to_string(), lines),
    }
}

// ---- best-effort rescan (unparsable source) -------------------------------
// A line-prefix scan, deliberately not a recovering parser (SPEC §11): it
// recognizes the three top-level symbol shapes and can misread free text
// that happens to share them (a `| def x` text-block line is skipped, but a
// raw-passthrough body line starting `+x` is not). Good enough for "the
// buffer is mid-keystroke" — the parse error is shown alongside.

fn rescan(lines: &[&str], file: Option<&Path>, vfs: &dyn Vfs) -> Analysis {
    let mut a = Analysis {
        defs: Vec::new(),
        calls: Vec::new(),
        includes: Vec::new(),
        warnings: Vec::new(),
        error: None,
    };
    for (idx, text) in lines.iter().enumerate() {
        let line = idx + 1;
        let t = text.trim_start_matches([' ', '\t']).trim_end();
        if let Some(rest) = t.strip_prefix("def") {
            let Some(rest) = rest.strip_prefix([' ', '\t']) else {
                continue;
            };
            let name: String = ident_prefix(rest.trim_start());
            if name.is_empty() {
                continue;
            }
            let raw = rest
                .trim_start()
                .strip_prefix(&name)
                .and_then(|r| r.strip_prefix('('))
                .map(rescan_params)
                .unwrap_or_default();
            let name_span = name_span(lines, line, "def", &name);
            let params = param_syms(raw, lines, &name_span);
            a.defs.push(DefSym {
                name: name.clone(),
                name_span,
                params,
                end_line: block_end_line(lines, line),
                file: None,
            });
        } else if let Some(rest) = t.strip_prefix("include") {
            let Some(path) = rest.strip_prefix([' ', '\t']) else {
                continue;
            };
            let path = path.trim();
            if path.is_empty() {
                continue;
            }
            a.includes.push(IncludeSym {
                path: path.to_string(),
                span: include_path_span(lines, line, path),
                resolved: file.and_then(|f| vfs.locate(&resolve::include_target(f, path))),
            });
        } else if let Some(rest) = t.strip_prefix('+') {
            let name = ident_prefix(rest);
            if name.is_empty() {
                continue;
            }
            a.calls.push(CallSym {
                name: name.clone(),
                name_span: name_span(lines, line, "+", &name),
                args: Vec::new(),
            });
        }
    }
    // Included files are usually intact while this buffer is mid-edit:
    // chase includes breadth-first (visited set, no error reporting) so
    // cross-file definitions stay available.
    if let Some(file) = file {
        let mut visited: Vec<PathBuf> = Vec::new();
        let mut queue: Vec<(PathBuf, String)> = a
            .includes
            .iter()
            .map(|i| (file.to_path_buf(), i.path.clone()))
            .collect();
        while let Some((from, path)) = queue.pop() {
            let Some(target) = vfs.locate(&resolve::include_target(&from, &path)) else {
                continue;
            };
            if visited.contains(&target) {
                continue;
            }
            visited.push(target.clone());
            a.defs.extend(file_defs(&target, vfs));
            let Ok(src) = vfs.read(&target) else {
                continue;
            };
            if let Ok((doc, _)) = parser::parse(&src, true, ShorthandPolicy::Auto) {
                for n in &doc.body {
                    if let Node::Include { path, .. } = n {
                        queue.push((target.clone(), path.clone()));
                    }
                }
            }
        }
    }
    a
}

/// Leading `[A-Za-z_][A-Za-z0-9_]*` of `s` (empty if none).
fn ident_prefix(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        let ok = c == '_' || c.is_ascii_alphabetic() || (i > 0 && c.is_ascii_digit());
        if !ok {
            break;
        }
        out.push(c);
    }
    out
}

/// Parameter names (and default texts) from the inside of a rescanned
/// `def`'s paren list: split on top-level whitespace, quote- and
/// brace-aware so `limit={ctx.pageSize - 1}` stays one parameter.
fn rescan_params(s: &str) -> Vec<(String, Option<String>)> {
    let mut toks: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) => {
                cur.push(c);
                if c == q {
                    quote = None;
                }
            }
            None => match c {
                '"' | '\'' => {
                    quote = Some(c);
                    cur.push(c);
                }
                '{' => {
                    depth += 1;
                    cur.push(c);
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    cur.push(c);
                }
                ')' if depth == 0 => break,
                ' ' | '\t' if depth == 0 => {
                    if !cur.is_empty() {
                        toks.push(std::mem::take(&mut cur));
                    }
                }
                _ => cur.push(c),
            },
        }
    }
    if !cur.is_empty() {
        toks.push(cur);
    }
    toks.iter()
        .filter_map(|t| {
            let (name, default) = match t.split_once('=') {
                Some((n, d)) => (n, Some(d.to_string())),
                None => (t.as_str(), None),
            };
            let id = ident_prefix(name);
            (!id.is_empty() && id == name).then_some((id, default))
        })
        .collect()
}
