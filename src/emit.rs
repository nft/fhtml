//! HTML emitter (SPEC §11). Two modes producing the same element tree:
//! `Pretty` (2-space indented) and `Min` (no inter-tag whitespace).

use crate::parser::{is_void, AttrValue, Element, Node};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Pretty,
    Min,
}

pub fn emit(nodes: &[Node], mode: Mode) -> String {
    let mut out = String::new();
    for node in nodes {
        emit_node(&mut out, node, 0, mode);
    }
    out
}

/// Entity-escaping per SPEC §11: attribute values escape `& < > "`; text
/// escapes only `& < >` (a literal `"` in a text node is valid HTML and
/// cheaper). Class names and raw passthrough are never escaped.
fn esc_attr(s: &str) -> String {
    esc(s, true)
}

fn esc_text(s: &str) -> String {
    esc(s, false)
}

fn esc(s: &str, quotes: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if quotes => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// Attribute order per SPEC §11: id, paren attrs in source order, merged class.
fn open_tag(el: &Element) -> String {
    let mut s = format!("<{}", el.tag);
    if let Some(id) = &el.id {
        s.push_str(&format!(" id=\"{}\"", esc_attr(id)));
    }
    for (name, value) in &el.attrs {
        match value {
            AttrValue::Bool => s.push_str(&format!(" {name}")),
            AttrValue::Str(v) => s.push_str(&format!(" {name}=\"{}\"", esc_attr(v))),
        }
    }
    if !el.classes.is_empty() {
        s.push_str(&format!(" class=\"{}\"", el.classes.join(" ")));
    }
    s.push('>');
    s
}

fn emit_node(out: &mut String, node: &Node, depth: usize, mode: Mode) {
    let ind = if mode == Mode::Pretty {
        "  ".repeat(depth)
    } else {
        String::new()
    };
    let nl = if mode == Mode::Pretty { "\n" } else { "" };

    match node {
        Node::Doctype => out.push_str(&format!("{ind}<!DOCTYPE html>{nl}")),
        Node::Comment { emit: false, .. } => {}
        Node::Comment { lines, emit: true } => {
            if lines.len() == 1 {
                out.push_str(&format!("{ind}<!-- {} -->{nl}", lines[0]));
            } else {
                out.push_str(&format!("{ind}<!-- {}{nl}", lines[0]));
                for l in &lines[1..] {
                    out.push_str(&format!("{ind}{l}{nl}"));
                }
                if mode == Mode::Min {
                    out.push('\n'); // keep comment lines apart even minified
                }
                out.push_str(&format!("{ind}-->{nl}"));
            }
        }
        Node::Raw(lines) => {
            // Verbatim, never minified (raw may be whitespace-sensitive, SPEC §8).
            for (i, l) in lines.iter().enumerate() {
                if mode == Mode::Min && i > 0 {
                    out.push('\n');
                }
                if l.is_empty() {
                    out.push_str(nl);
                } else {
                    out.push_str(&format!("{ind}{l}{nl}"));
                }
            }
        }
        Node::TextBlock(lines) => {
            // Lines stay separate: the newline is content, collapsed to one
            // space by the browser (SPEC §6.2) — required in Min mode too,
            // otherwise adjacent lines would glue into one word.
            for (i, l) in lines.iter().enumerate() {
                if mode == Mode::Min && i > 0 {
                    out.push('\n');
                }
                out.push_str(&format!("{ind}{}{nl}", esc_text(l)));
            }
        }
        Node::Element(el) => emit_element(out, el, depth, mode),
    }
}

fn emit_element(out: &mut String, el: &Element, depth: usize, mode: Mode) {
    // A `>` chain renders glued on one line: opens (each followed by its own
    // inline text), then either the innermost's children as a block, or the
    // closings immediately (SPEC §4.6, §5).
    let mut opens = String::new();
    let mut closings = String::new();
    let mut cur = el;
    loop {
        opens.push_str(&open_tag(cur));
        if let Some(text) = &cur.text {
            opens.push_str(&esc_text(text));
        }
        if !is_void(&cur.tag) {
            closings = format!("</{}>{}", cur.tag, closings);
        }
        match &cur.chain {
            Some(next) => cur = next,
            None => break,
        }
    }
    let inner = cur;

    let ind = if mode == Mode::Pretty {
        "  ".repeat(depth)
    } else {
        String::new()
    };
    let nl = if mode == Mode::Pretty { "\n" } else { "" };

    if inner.children.is_empty() {
        out.push_str(&format!("{ind}{opens}{closings}{nl}"));
    } else {
        out.push_str(&format!("{ind}{opens}{nl}"));
        for child in &inner.children {
            emit_node(out, child, depth + 1, mode);
        }
        out.push_str(&format!("{ind}{closings}{nl}"));
    }
}
