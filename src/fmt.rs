//! Canonical formatter: 2-space indentation, spaces only, `.` for `div`,
//! minimal quoting. Invariant: `compile(format(src)) == compile(src)`.
//! Silent `//` comments are preserved (they live in the AST for this reason).

use crate::parser::{AttrValue, Element, Node};

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
            for l in lines {
                if l.is_empty() {
                    out.push_str(&format!("{ind}|\n"));
                } else {
                    out.push_str(&format!("{ind}| {}\n", l.replace('{', "\\{")));
                }
            }
        }
        Node::Element(el) => {
            out.push_str(&format!("{ind}{}\n", element_line(el)));
            for child in &innermost(el).children {
                fmt_node(out, child, depth + 1);
            }
        }
    }
}

fn innermost(el: &Element) -> &Element {
    match &el.chain {
        Some(next) => innermost(next),
        None => el,
    }
}

/// A class that, printed bare, would reparse as a different token kind
/// (text, id, interpolation, or chain). Such classes can only enter the AST
/// via a `class="…"` attribute, and must leave the same way.
fn hostile_class(c: &str) -> bool {
    c.starts_with('"') || c.starts_with('{') || c.starts_with('#') || c == ">"
}

fn element_line(el: &Element) -> String {
    let mut s = String::new();
    s.push_str(if el.tag == "div" { "." } else { &el.tag });
    // If any class can't survive as a bare token, the whole list rides in a
    // quoted class attr (the parser merges it back losslessly, in order).
    let class_attr = if el.classes.iter().any(|c| hostile_class(c)) {
        Some(el.classes.join(" "))
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
            if let AttrValue::Str(v) = value {
                s.push('=');
                s.push_str(&attr_value(v));
            }
        }
        if let Some(v) = &class_attr {
            if !first {
                s.push(' ');
            }
            s.push_str("class=");
            s.push_str(&attr_value(v));
        }
        s.push(')');
    }
    if let Some(id) = &el.id {
        s.push_str(&format!(" #{id}"));
    }
    if class_attr.is_none() {
        for class in &el.classes {
            s.push(' ');
            s.push_str(class);
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
fn attr_value(v: &str) -> String {
    let needs_quoting =
        v.is_empty() || v.starts_with('{') || v.contains([' ', '\t', ')', '"', '\'', '\\']);
    if needs_quoting {
        let mut s = String::from("\"");
        for c in v.chars() {
            match c {
                '\\' => s.push_str("\\\\"),
                '"' => s.push_str("\\\""),
                '{' => s.push_str("\\{"),
                _ => s.push(c),
            }
        }
        s.push('"');
        s
    } else {
        v.to_string()
    }
}

fn quoted_text(t: &str) -> String {
    let mut s = String::from("\"");
    for c in t.chars() {
        match c {
            '\\' => s.push_str("\\\\"),
            '"' => s.push_str("\\\""),
            '{' => s.push_str("\\{"),
            '\n' => s.push_str("\\n"),
            _ => s.push(c),
        }
    }
    s.push('"');
    s
}
