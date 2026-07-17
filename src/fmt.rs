//! Canonical formatter: 2-space indentation, spaces only, `.` for `div`,
//! minimal quoting. Invariant: `compile(format(src)) == compile(src)` (and
//! `render` likewise). Silent `//` comments are preserved (they live in the
//! AST for this reason). Expressions are reprinted from their trimmed source
//! text, never re-serialized from the AST — formatting must not change output.

use crate::parser::{AttrValue, Call, ClassItem, Def, Document, Element, IfChain, Node, TextPart};
use crate::FmtShorthand;

/// How literal class tokens reprint (SPEC §3.2). `Preserve` is the identity;
/// the rewrite modes first resolve what the authored token *means* under the
/// document's own directive state, then re-emit that meaning in the target
/// form — so both directions are output-preserving on any input file.
#[derive(Clone, Copy)]
struct Classes {
    /// The document opened with `#!shorthand`, so authored tokens carry
    /// decoded meanings (`fx` is `flex`, `=fx` is the literal `fx`).
    decoded: bool,
    mode: FmtShorthand,
}

impl Classes {
    fn apply(&self, tok: &str) -> String {
        if self.mode == FmtShorthand::Preserve {
            return tok.to_string();
        }
        let meaning = crate::parser::class_token(tok, self.decoded);
        match self.mode {
            FmtShorthand::Preserve => unreachable!(),
            // Verbatim form: without a directive every token is literal.
            FmtShorthand::Expand => meaning,
            // Shorthand form: codes where they round-trip, `=`-escapes where
            // the class would read as something else.
            FmtShorthand::Contract => crate::shorthand::contract(&meaning),
        }
    }
}

pub fn format_document(doc: &Document, shorthand: FmtShorthand) -> String {
    let mut out = String::new();
    // The authored `#!shorthand` opt-in survives `Preserve` (SPEC §3.2);
    // `format()` parses with decoding off, so class tokens are still the
    // authored codes and the pair round-trips byte-identically. The rewrite
    // modes set the directive to match the form they emit.
    let directive = match shorthand {
        FmtShorthand::Preserve => doc.shorthand,
        FmtShorthand::Expand => false,
        FmtShorthand::Contract => true,
    };
    if directive {
        out.push_str("#!shorthand\n");
    }
    let classes = Classes {
        decoded: doc.shorthand,
        mode: shorthand,
    };
    for node in &doc.body {
        match node {
            // Definitions print where they sat in the source (SPEC §10.3).
            Node::DefSite(i) => fmt_def(&mut out, &doc.defs[*i], classes),
            // Reprinted as written (SPEC §10.5) — fmt never resolves
            // includes; formatting must not need the filesystem.
            Node::Include { path, .. } => {
                out.push_str("include ");
                out.push_str(path);
                out.push('\n');
            }
            _ => fmt_node(&mut out, node, 0, classes),
        }
    }
    out
}

/// Formats a plain node list — the converter's output. `DefSite` markers
/// cannot occur here: they only arise from parsing `def` lines.
#[cfg(feature = "convert")]
pub fn format_nodes(nodes: &[Node]) -> String {
    let mut out = String::new();
    let classes = Classes {
        decoded: false,
        mode: FmtShorthand::Preserve,
    };
    for node in nodes {
        fmt_node(&mut out, node, 0, classes);
    }
    out
}

/// `def name(param param=default …)`, body indented one step. Empty parameter
/// lists print bare (`def name`) — the parse is identical.
fn fmt_def(out: &mut String, def: &Def, classes: Classes) {
    out.push_str("def ");
    out.push_str(&def.name);
    if !def.params.is_empty() {
        out.push('(');
        for (i, p) in def.params.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            out.push_str(&p.name);
            if let Some(d) = &p.default {
                out.push('=');
                out.push_str(&expr_value(&d.src));
            }
        }
        out.push(')');
    }
    out.push('\n');
    for child in &def.body {
        fmt_node(out, child, 1, classes);
    }
}

fn fmt_node(out: &mut String, node: &Node, depth: usize, classes: Classes) {
    let ind = "  ".repeat(depth);
    match node {
        Node::Doctype => out.push_str(&format!("{ind}doctype\n")),
        Node::Comment { lines, emit } => {
            let sigil = if *emit { "//!" } else { "//" };
            if lines[0].is_empty() {
                out.push_str(&format!("{ind}{sigil}\n"));
            } else {
                out.push_str(&format!("{ind}{sigil} {}\n", lines[0]));
            }
            // Block lines keep their stored relative indent, re-anchored here.
            for l in &lines[1..] {
                if l.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("{ind}{l}\n"));
                }
            }
        }
        Node::Raw(lines) => {
            out.push_str(&format!("{ind}{}\n", lines[0]));
            for l in &lines[1..] {
                if l.is_empty() {
                    out.push('\n');
                } else {
                    out.push_str(&format!("{ind}{l}\n"));
                }
            }
        }
        Node::TextBlock(lines) => {
            for parts in lines {
                if parts.is_empty() {
                    out.push_str(&format!("{ind}|\n"));
                } else {
                    out.push_str(&format!("{ind}| {}\n", block_text(parts)));
                }
            }
        }
        Node::Element(el) => {
            out.push_str(&format!("{ind}{}\n", element_line(el, classes)));
            let inner = innermost(el);
            if let Some(body) = &inner.raw_body {
                // Raw-text body (SPEC §6.3): content bytes reprint verbatim —
                // no reindent inside the `|`, no escape rewriting.
                let cind = "  ".repeat(depth + 1);
                for l in body {
                    if l.is_empty() {
                        out.push_str(&format!("{cind}|\n"));
                    } else {
                        out.push_str(&format!("{cind}| {l}\n"));
                    }
                }
            }
            for child in &inner.children {
                fmt_node(out, child, depth + 1, classes);
            }
        }
        Node::Call(c) => {
            out.push_str(&format!("{ind}{}\n", call_line(c)));
            for child in &c.children {
                fmt_node(out, child, depth + 1, classes);
            }
        }
        Node::Children { .. } => out.push_str(&format!("{ind}children\n")),
        // Top level only (parser-enforced) — `format_document` handles them.
        Node::DefSite(_) => unreachable!("`def` is top-level only (SPEC §10.3)"),
        Node::Include { .. } => unreachable!("`include` is top-level only (SPEC §10.5)"),
        Node::If(chain) => fmt_if(out, chain, depth, classes),
        Node::For(f) => {
            match &f.index {
                Some(idx) => {
                    out.push_str(&format!("{ind}for {}, {idx} in {}\n", f.var, f.iter.src))
                }
                None => out.push_str(&format!("{ind}for {} in {}\n", f.var, f.iter.src)),
            }
            for child in &f.body {
                fmt_node(out, child, depth + 1, classes);
            }
            if let Some(empty) = &f.empty {
                out.push_str(&format!("{ind}empty\n"));
                for child in empty {
                    fmt_node(out, child, depth + 1, classes);
                }
            }
        }
    }
}

fn fmt_if(out: &mut String, chain: &IfChain, depth: usize, classes: Classes) {
    let ind = "  ".repeat(depth);
    for (i, arm) in chain.arms.iter().enumerate() {
        let kw = if i == 0 { "if" } else { "elif" };
        out.push_str(&format!("{ind}{kw} {}\n", arm.cond.src));
        for child in &arm.body {
            fmt_node(out, child, depth + 1, classes);
        }
    }
    if let Some(else_body) = &chain.else_body {
        out.push_str(&format!("{ind}else\n"));
        for child in else_body {
            fmt_node(out, child, depth + 1, classes);
        }
    }
}

/// `+name(arg arg=value …)`; empty argument lists print bare (`+name`).
fn call_line(c: &Call) -> String {
    let mut s = format!("+{}", c.name);
    if !c.args.is_empty() {
        s.push('(');
        for (i, arg) in c.args.iter().enumerate() {
            if i > 0 {
                s.push(' ');
            }
            s.push_str(&arg.name);
            match &arg.value {
                AttrValue::Bool => {}
                // Always quoted — printed bare, the value would reparse as
                // an expression, not a string (SPEC §10.4).
                AttrValue::Str(parts) => {
                    s.push('=');
                    s.push_str(&quoted_string(parts));
                }
                AttrValue::Expr(t) => {
                    s.push('=');
                    s.push_str(&expr_value(&t.src));
                }
            }
        }
        s.push(')');
    }
    s
}

/// An expression value after `=` in a parameter default or call argument
/// (SPEC §10.3–§10.4). Bare when the reparse reads it back identically;
/// braced when the source contains whitespace or `)` (the unquoted form
/// stops there) or starts with a quote (which would reparse as a string
/// argument).
fn expr_value(src: &str) -> String {
    if src.contains([' ', '\t', ')']) || src.starts_with(['"', '\'']) {
        interp(src)
    } else {
        src.to_string()
    }
}

/// A non-raw `{expr}`. A leading `!` would reparse as the raw sigil `{!…}`
/// (rejected in attributes and classes, unescaped in text), so it keeps one
/// space after the brace — `{ !expr}` trims back to the same expression.
fn interp(src: &str) -> String {
    if src.starts_with('!') {
        format!("{{ {src}}}")
    } else {
        format!("{{{src}}}")
    }
}

fn innermost(el: &Element) -> &Element {
    match &el.chain {
        Some(next) => innermost(next),
        None => el,
    }
}

/// A literal class that, printed bare, would reparse as a different token kind
/// (text, interpolation, id, or chain). Such classes can only enter the AST
/// via a `class="…"` attribute, and must leave the same way.
fn hostile_class(c: &str) -> bool {
    c.starts_with('"') || c.starts_with('{') || c.starts_with('#') || c == ">"
}

fn element_line(el: &Element, cm: Classes) -> String {
    let mut s = String::new();
    s.push_str(if el.tag == "div" { "." } else { &el.tag });
    // The shorthand rewrite happens before the hostility check: Expand can
    // *create* a hostile token (`=#foo` under the directive means the literal
    // class `#foo`, which only survives inside a quoted class attr).
    let classes: Vec<ClassItem> = el
        .classes
        .iter()
        .map(|c| match c {
            ClassItem::Lit(s) => ClassItem::Lit(cm.apply(s)),
            interp @ ClassItem::Interp(_) => interp.clone(),
        })
        .collect();
    // If any literal class can't survive as a bare token, the whole list rides
    // in a quoted class attr (the parser merges it back losslessly, in order;
    // interpolations print as whitespace-separated `{expr}` inside it).
    let hostile = classes
        .iter()
        .any(|c| matches!(c, ClassItem::Lit(s) if hostile_class(s)));
    let class_attr = if hostile {
        Some(
            classes
                .iter()
                .map(|c| match c {
                    ClassItem::Lit(s) => escape_attr_lit(s),
                    ClassItem::Interp(t) => interp(&t.src),
                })
                .collect::<Vec<_>>()
                .join(" "),
        )
    } else {
        None
    };
    if !el.attrs.is_empty() || class_attr.is_some() {
        s.push('(');
        let mut first = true;
        for (name, value) in &el.attrs {
            if !first {
                s.push(' ');
            }
            first = false;
            s.push_str(name);
            match value {
                AttrValue::Bool => {}
                AttrValue::Str(parts) => {
                    s.push('=');
                    s.push_str(&attr_value(parts));
                }
                AttrValue::Expr(t) => {
                    s.push('=');
                    s.push_str(&interp(&t.src));
                }
            }
        }
        if let Some(v) = &class_attr {
            if !first {
                s.push(' ');
            }
            s.push_str(&format!("class=\"{v}\""));
        }
        s.push(')');
    }
    if let Some(id) = &el.id {
        s.push_str(&format!(" #{id}"));
    }
    if class_attr.is_none() {
        for class in &classes {
            s.push(' ');
            match class {
                ClassItem::Lit(c) => s.push_str(c),
                ClassItem::Interp(t) => s.push_str(&interp(&t.src)),
            }
        }
    }
    if let Some(text) = &el.text {
        s.push(' ');
        s.push_str(&quoted_text(text));
    }
    if let Some(chain) = &el.chain {
        s.push_str(" > ");
        s.push_str(&element_line(chain, cm));
    }
    s
}

/// Bare when the reparse would read it back identically; quoted otherwise.
/// Segments with interpolation always print quoted.
fn attr_value(parts: &[TextPart]) -> String {
    if let [TextPart::Lit(v)] = parts {
        let needs_quoting =
            v.is_empty() || v.starts_with('{') || v.contains([' ', '\t', ')', '"', '\'', '\\']);
        if !needs_quoting {
            return v.to_string();
        }
    }
    quoted_string(parts)
}

/// A string value in its always-quoted form: attribute values that need
/// quoting, and call-argument strings (which may never print bare —
/// SPEC §10.4 reads unquoted values as expressions). Raw `{!…}` cannot
/// occur here (forbidden in attribute-shaped values, SPEC §9.1).
fn quoted_string(parts: &[TextPart]) -> String {
    let mut s = String::from("\"");
    for part in parts {
        match part {
            TextPart::Lit(v) => s.push_str(&escape_attr_lit(v)),
            TextPart::Interp { expr, .. } => s.push_str(&interp(&expr.src)),
        }
    }
    s.push('"');
    s
}

fn escape_attr_lit(v: &str) -> String {
    let mut s = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '{' => s.push_str("\\{"),
            _ => s.push(c),
        }
    }
    s
}

fn quoted_text(parts: &[TextPart]) -> String {
    let mut s = String::from("\"");
    for part in parts {
        match part {
            TextPart::Lit(t) => {
                for c in t.chars() {
                    match c {
                        '\\' => s.push_str("\\\\"),
                        '"' => s.push_str("\\\""),
                        '{' => s.push_str("\\{"),
                        '\n' => s.push_str("\\n"),
                        _ => s.push(c),
                    }
                }
            }
            TextPart::Interp { expr, raw } => {
                if *raw {
                    s.push_str(&format!("{{!{}}}", expr.src));
                } else {
                    s.push_str(&interp(&expr.src));
                }
            }
        }
    }
    s.push('"');
    s
}

/// A `|` line's content: literal except `\{` escapes and interpolation.
fn block_text(parts: &[TextPart]) -> String {
    let mut s = String::new();
    for part in parts {
        match part {
            TextPart::Lit(t) => s.push_str(&t.replace('{', "\\{")),
            TextPart::Interp { expr, raw } => {
                if *raw {
                    s.push_str(&format!("{{!{}}}", expr.src));
                } else {
                    s.push_str(&interp(&expr.src));
                }
            }
        }
    }
    s
}
