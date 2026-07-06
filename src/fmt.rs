//! Canonical formatter: 2-space indentation, spaces only, `.` for `div`,
//! minimal quoting. Invariant: `compile(format(src)) == compile(src)` (and
//! `render` likewise). Silent `//` comments are preserved (they live in the
//! AST for this reason). Expressions are reprinted from their trimmed source
//! text, never re-serialized from the AST — formatting must not change output.

use crate::parser::{AttrValue, ClassItem, Element, IfChain, Node, TextPart};

pub fn format_nodes(nodes: &[Node]) -> String {
    let mut out = String::new();
    for node in nodes {
        fmt_node(&mut out, node, 0);
    }
    out
}

fn fmt_node(out: &mut String, node: &Node, depth: usize) {
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
            out.push_str(&format!("{ind}{}\n", element_line(el)));
            for child in &innermost(el).children {
                fmt_node(out, child, depth + 1);
            }
        }
        Node::If(chain) => fmt_if(out, chain, depth),
        Node::For(f) => {
            match &f.index {
                Some(idx) => {
                    out.push_str(&format!("{ind}for {}, {idx} in {}\n", f.var, f.iter.src))
                }
                None => out.push_str(&format!("{ind}for {} in {}\n", f.var, f.iter.src)),
            }
            for child in &f.body {
                fmt_node(out, child, depth + 1);
            }
            if let Some(empty) = &f.empty {
                out.push_str(&format!("{ind}empty\n"));
                for child in empty {
                    fmt_node(out, child, depth + 1);
                }
            }
        }
    }
}

fn fmt_if(out: &mut String, chain: &IfChain, depth: usize) {
    let ind = "  ".repeat(depth);
    for (i, arm) in chain.arms.iter().enumerate() {
        let kw = if i == 0 { "if" } else { "elif" };
        out.push_str(&format!("{ind}{kw} {}\n", arm.cond.src));
        for child in &arm.body {
            fmt_node(out, child, depth + 1);
        }
    }
    if let Some(else_body) = &chain.else_body {
        out.push_str(&format!("{ind}else\n"));
        for child in else_body {
            fmt_node(out, child, depth + 1);
        }
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

fn element_line(el: &Element) -> String {
    let mut s = String::new();
    s.push_str(if el.tag == "div" { "." } else { &el.tag });
    // If any literal class can't survive as a bare token, the whole list rides
    // in a quoted class attr (the parser merges it back losslessly, in order;
    // interpolations print as whitespace-separated `{expr}` inside it).
    let hostile = el
        .classes
        .iter()
        .any(|c| matches!(c, ClassItem::Lit(s) if hostile_class(s)));
    let class_attr = if hostile {
        Some(
            el.classes
                .iter()
                .map(|c| match c {
                    ClassItem::Lit(s) => escape_attr_lit(s),
                    ClassItem::Interp(t) => format!("{{{}}}", t.src),
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
                    s.push_str(&format!("={{{}}}", t.src));
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
        for class in &el.classes {
            s.push(' ');
            match class {
                ClassItem::Lit(c) => s.push_str(c),
                ClassItem::Interp(t) => s.push_str(&format!("{{{}}}", t.src)),
            }
        }
    }
    if let Some(text) = &el.text {
        s.push(' ');
        s.push_str(&quoted_text(text));
    }
    if let Some(chain) = &el.chain {
        s.push_str(" > ");
        s.push_str(&element_line(chain));
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
    let mut s = String::from("\"");
    for part in parts {
        match part {
            TextPart::Lit(v) => s.push_str(&escape_attr_lit(v)),
            TextPart::Interp { expr, .. } => s.push_str(&format!("{{{}}}", expr.src)),
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
                let bang = if *raw { "!" } else { "" };
                s.push_str(&format!("{{{bang}{}}}", expr.src));
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
                let bang = if *raw { "!" } else { "" };
                s.push_str(&format!("{{{bang}{}}}", expr.src));
            }
        }
    }
    s
}
