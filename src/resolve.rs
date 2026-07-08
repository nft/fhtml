//! Include resolution (SPEC §10.5): loads each `include <path>` target,
//! splices its top-level markup at the include site, and merges its `def`s
//! into the one shared namespace. Runs between parsing and rendering/codegen
//! so `emit` and `jsgen` never see a [`Node::Include`] — and never runs for
//! `fhtml fmt`, which reprints include lines as written.
//!
//! Error attribution across files: everything caught while *loading* (a
//! missing file, a parse error inside the included file, a cycle, a `def`
//! collision) is reported at the `include` line of the including file, with
//! the included file's own path and position in the message. Errors that
//! surface later — render-time evaluation inside included content — must
//! carry a position that exists in the root file, byte-identically between
//! the native renderer and the compiled JS module; so every position in
//! included content is remapped to the include site (line, column 1). The
//! trade-off is deliberate v0.1: a coarse-but-honest location over a precise
//! line in the wrong file.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::{err, Error, Result};
use crate::parser::{
    self, Arg, AttrValue, ClassItem, Def, Document, Element, Node, TextPart, TplExpr,
};

/// Splices every `include` in `doc` (recursively). `file` is the path the
/// source was read from — the base for relative include paths and the first
/// link of cycle chains. A document with no includes passes through untouched
/// (so string-only entry points keep working); with includes and no `file`,
/// this is the "stdin has no base path" error.
pub(crate) fn resolve_includes(
    doc: Document,
    file: Option<&Path>,
    warnings: &mut Vec<String>,
) -> Result<Document> {
    let Some((line, path)) = first_include(&doc.body) else {
        return Ok(doc);
    };
    let Some(file) = file else {
        return err(
            line,
            1,
            format!(
                "cannot resolve `include {path}` — the source has no file path (stdin?); includes are relative to the including file (SPEC §10.5)"
            ),
        );
    };
    let mut stack = vec![(canon(file), file.display().to_string())];
    expand(doc, file, &mut stack, warnings)
}

fn first_include(nodes: &[Node]) -> Option<(usize, &str)> {
    nodes.iter().find_map(|n| match n {
        Node::Include { path, line } => Some((*line, path.as_str())),
        _ => None,
    })
}

/// Identity for cycle detection. Canonicalization only fails for paths that
/// don't exist — and the target is read before this is called — so the
/// fallback is defensive.
fn canon(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Rewrites an error from inside an included file to the include site,
/// keeping the inner path and position in the message. Nested includes
/// compose: each level adds its own `in `…`` prefix.
fn at_include(line: usize, display: &str, e: Error) -> Error {
    Error {
        line,
        col: 1,
        msg: format!("in `{display}`: {}:{}: {}", e.line, e.col, e.msg),
    }
}

fn expand(
    doc: Document,
    file: &Path,
    stack: &mut Vec<(PathBuf, String)>,
    warnings: &mut Vec<String>,
) -> Result<Document> {
    let mut defs = doc.defs;
    let mut body = Vec::with_capacity(doc.body.len());
    for node in doc.body {
        let Node::Include { path, line } = node else {
            body.push(node);
            continue;
        };
        // `.fhtml` appended if absent (SPEC §10.5), relative to this file.
        // The idiomatic leading `./` is dropped so displayed paths (cycle
        // chains, error prefixes) stay clean.
        let bare = path.trim_start_matches("./");
        let rel = if bare.ends_with(".fhtml") {
            bare.to_string()
        } else {
            format!("{bare}.fhtml")
        };
        let target = file.parent().unwrap_or(Path::new("")).join(&rel);
        let display = target.display().to_string();
        let src = fs::read_to_string(&target).map_err(|e| Error {
            line,
            col: 1,
            msg: format!("cannot include `{display}`: {e}"),
        })?;
        let id = canon(&target);
        if let Some(pos) = stack.iter().position(|(c, _)| *c == id) {
            let chain: Vec<&str> = stack[pos..]
                .iter()
                .map(|(_, d)| d.as_str())
                .chain([display.as_str()])
                .collect();
            return err(
                line,
                1,
                format!("include cycle: {} (SPEC §10.5)", chain.join(" -> ")),
            );
        }
        let (idoc, iwarnings) =
            parser::parse(&src, true).map_err(|e| at_include(line, &display, e))?;
        for w in iwarnings {
            warnings.push(format!("`{display}`:{w}"));
        }
        stack.push((id, display.clone()));
        let mut idoc = expand(idoc, &target, stack, warnings).map_err(|e| {
            // A nested failure already names its own file; re-anchor its
            // position (a line in `target`) to this include site.
            at_include(line, &display, e)
        })?;
        stack.pop();

        let offset = defs.len();
        for mut d in std::mem::take(&mut idoc.defs) {
            if let Some(prev) = defs.iter().find(|x| x.name == d.name) {
                return err(
                    line,
                    1,
                    format!(
                        "`include {path}` defines component `{}`, which is already defined (line {}) — component names share one namespace across includes (SPEC §10.5)",
                        d.name, prev.line
                    ),
                );
            }
            remap_def(&mut d, line);
            defs.push(d);
        }
        for mut n in idoc.body {
            if let Node::DefSite(i) = &mut n {
                *i += offset;
            }
            remap_node(&mut n, line);
            body.push(n);
        }
    }
    Ok(Document { defs, body })
}

// ---- position remap ------------------------------------------------------
// Included content reports errors at the include site (module doc above).

fn remap_expr(t: &mut TplExpr, line: usize) {
    t.line = line;
    t.col = 1;
}

fn remap_parts(parts: &mut [TextPart], line: usize) {
    for p in parts {
        if let TextPart::Interp { expr, .. } = p {
            remap_expr(expr, line);
        }
    }
}

fn remap_attr_value(v: &mut AttrValue, line: usize) {
    match v {
        AttrValue::Bool => {}
        AttrValue::Str(parts) => remap_parts(parts, line),
        AttrValue::Expr(t) => remap_expr(t, line),
    }
}

fn remap_element(el: &mut Element, line: usize) {
    el.line = line;
    for c in &mut el.classes {
        if let ClassItem::Interp(t) = c {
            remap_expr(t, line);
        }
    }
    for (_, v) in &mut el.attrs {
        remap_attr_value(v, line);
    }
    if let Some(text) = &mut el.text {
        remap_parts(text, line);
    }
    if let Some(chain) = &mut el.chain {
        remap_element(chain, line);
    }
    remap_nodes(&mut el.children, line);
}

fn remap_arg(a: &mut Arg, line: usize) {
    a.line = line;
    a.col = 1;
    remap_attr_value(&mut a.value, line);
}

fn remap_def(d: &mut Def, line: usize) {
    d.line = line;
    for p in &mut d.params {
        if let Some(t) = &mut p.default {
            remap_expr(t, line);
        }
    }
    remap_nodes(&mut d.body, line);
}

fn remap_nodes(nodes: &mut [Node], line: usize) {
    for n in nodes {
        remap_node(n, line);
    }
}

fn remap_node(n: &mut Node, line: usize) {
    match n {
        Node::Element(el) => remap_element(el, line),
        Node::TextBlock(lines) => {
            for parts in lines {
                remap_parts(parts, line);
            }
        }
        Node::Raw(_) | Node::Comment { .. } | Node::Doctype | Node::DefSite(_) => {}
        Node::If(chain) => {
            chain.line = line;
            for arm in &mut chain.arms {
                remap_expr(&mut arm.cond, line);
                remap_nodes(&mut arm.body, line);
            }
            if let Some(body) = &mut chain.else_body {
                remap_nodes(body, line);
            }
        }
        Node::For(f) => {
            f.line = line;
            remap_expr(&mut f.iter, line);
            remap_nodes(&mut f.body, line);
            if let Some(empty) = &mut f.empty {
                remap_nodes(empty, line);
            }
        }
        Node::Call(c) => {
            c.line = line;
            for a in &mut c.args {
                remap_arg(a, line);
            }
            remap_nodes(&mut c.children, line);
        }
        Node::Children { line: l } => *l = line,
        Node::Include { .. } => unreachable!("nested docs are expanded bottom-up"),
    }
}
