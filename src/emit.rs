//! Renderer (SPEC §9–§11). Two modes producing the same element tree:
//! `Pretty` (2-space indented) and `Min` (no inter-tag whitespace).
//!
//! One code path serves both the static and template layers: `compile`
//! rejects template constructs and then renders with null data (a
//! literal-only tree evaluates nothing, so its output is byte-identical to
//! the static emitter and cannot error); `render` evaluates statements and
//! interpolation against caller data. Render errors carry the file position
//! of the interpolation or statement, in the same format as parse errors.

use crate::error::{err, Error, Result};
use crate::expr::{self, Scope, Value};
use crate::parser::{is_void, AttrValue, ClassItem, Element, ForLoop, Node, TextPart, TplExpr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Pretty,
    Min,
}

/// Renders nodes against `data` (the name-resolution root) and `ctx` (the
/// reserved context root, SPEC §9.4). Pass `Value::Null` for an empty scope.
pub fn render_nodes(nodes: &[Node], mode: Mode, data: &Value, ctx: &Value) -> Result<String> {
    let mut r = R {
        mode,
        scope: Scope::new(data),
        ctx,
        out: String::new(),
    };
    r.block(nodes, 0)?;
    Ok(r.out)
}

/// Entity-escaping per SPEC §11: attribute values escape `& < > "`; text
/// escapes only `& < >` (a literal `"` in a text node is valid HTML and
/// cheaper). Literal class names and raw passthrough are never escaped.
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

struct R<'a> {
    mode: Mode,
    scope: Scope<'a>,
    ctx: &'a Value,
    out: String,
}

impl R<'_> {
    fn eval(&self, t: &TplExpr) -> Result<Value> {
        expr::eval(&t.expr, &self.scope, self.ctx).map_err(|e| Error {
            line: t.line,
            col: t.col,
            msg: e.msg,
        })
    }

    /// Evaluate + stringify (SPEC §9.4; lists/maps error at the site).
    fn eval_string(&self, t: &TplExpr) -> Result<String> {
        let v = self.eval(t)?;
        expr::stringify(&v).map_err(|e| Error {
            line: t.line,
            col: t.col,
            msg: e.msg,
        })
    }

    /// Text segments → output text. `quotes` selects attribute escaping
    /// (`& < > "`) over text escaping (`& < >`); raw parts bypass both.
    fn text(&self, parts: &[TextPart], quotes: bool) -> Result<String> {
        let mut s = String::new();
        for part in parts {
            match part {
                TextPart::Lit(l) => s.push_str(&esc(l, quotes)),
                TextPart::Interp { expr, raw } => {
                    let v = self.eval_string(expr)?;
                    if *raw {
                        s.push_str(&v);
                    } else {
                        s.push_str(&esc(&v, quotes));
                    }
                }
            }
        }
        Ok(s)
    }

    /// The final class-name list: literal tokens byte-for-byte (`"` as
    /// `&quot;`, SPEC §11); interpolation results split on whitespace and
    /// attribute-escaped (SPEC §9.1).
    fn class_names(&self, el: &Element) -> Result<Vec<String>> {
        let mut names = Vec::new();
        for item in &el.classes {
            match item {
                ClassItem::Lit(c) => names.push(c.replace('"', "&quot;")),
                ClassItem::Interp(t) => {
                    let s = self.eval_string(t)?;
                    names.extend(s.split_whitespace().map(|c| esc(c, true)));
                }
            }
        }
        Ok(names)
    }

    /// Attribute order per SPEC §11: id, paren attrs in source order, merged class.
    fn open_tag(&self, el: &Element) -> Result<String> {
        let mut s = format!("<{}", el.tag);
        if let Some(id) = &el.id {
            s.push_str(&format!(" id=\"{}\"", esc(id, true)));
        }
        for (name, value) in &el.attrs {
            match value {
                AttrValue::Bool => s.push_str(&format!(" {name}")),
                AttrValue::Str(v) => s.push_str(&format!(" {name}=\"{}\"", self.text(v, true)?)),
                AttrValue::Expr(t) => {
                    s.push_str(&format!(" {name}=\"{}\"", esc(&self.eval_string(t)?, true)))
                }
            }
        }
        let classes = self.class_names(el)?;
        if !classes.is_empty() {
            s.push_str(&format!(" class=\"{}\"", classes.join(" ")));
        }
        s.push('>');
        Ok(s)
    }

    fn block(&mut self, nodes: &[Node], depth: usize) -> Result<()> {
        for node in nodes {
            self.node(node, depth)?;
        }
        Ok(())
    }

    fn node(&mut self, node: &Node, depth: usize) -> Result<()> {
        let ind = if self.mode == Mode::Pretty {
            "  ".repeat(depth)
        } else {
            String::new()
        };
        let nl = if self.mode == Mode::Pretty { "\n" } else { "" };

        match node {
            Node::Doctype => self.out.push_str(&format!("{ind}<!DOCTYPE html>{nl}")),
            Node::Comment { emit: false, .. } => {}
            Node::Comment { lines, emit: true } => {
                if lines.len() == 1 {
                    self.out
                        .push_str(&format!("{ind}<!-- {} -->{nl}", lines[0]));
                } else {
                    self.out.push_str(&format!("{ind}<!-- {}{nl}", lines[0]));
                    for l in &lines[1..] {
                        self.out.push_str(&format!("{ind}{l}{nl}"));
                    }
                    if self.mode == Mode::Min {
                        self.out.push('\n'); // keep comment lines apart even minified
                    }
                    self.out.push_str(&format!("{ind}-->{nl}"));
                }
            }
            Node::Raw(lines) => {
                // Verbatim, never minified (raw may be whitespace-sensitive, SPEC §8).
                for (i, l) in lines.iter().enumerate() {
                    if self.mode == Mode::Min && i > 0 {
                        self.out.push('\n');
                    }
                    if l.is_empty() {
                        self.out.push_str(nl);
                    } else {
                        self.out.push_str(&format!("{ind}{l}{nl}"));
                    }
                }
            }
            Node::TextBlock(lines) => {
                // Lines stay separate: the newline is content, collapsed to one
                // space by the browser (SPEC §6.2) — required in Min mode too,
                // otherwise adjacent lines would glue into one word.
                for (i, parts) in lines.iter().enumerate() {
                    if self.mode == Mode::Min && i > 0 {
                        self.out.push('\n');
                    }
                    let text = self.text(parts, false)?;
                    self.out.push_str(&format!("{ind}{text}{nl}"));
                }
            }
            Node::Element(el) => self.element(el, depth)?,
            // Statements are invisible in the output tree: bodies render at
            // the statement's own depth.
            Node::If(chain) => {
                for arm in &chain.arms {
                    if self.eval(&arm.cond)?.truthy() {
                        return self.block(&arm.body, depth);
                    }
                }
                if let Some(body) = &chain.else_body {
                    self.block(body, depth)?;
                }
            }
            Node::For(f) => self.for_loop(f, depth)?,
            // stub: calls parse but
            // component rendering is not implemented yet.
            Node::Call(c) => {
                return err(
                    c.line,
                    1,
                    format!(
                        "`+{}` parses, but component rendering is not implemented yet",
                        c.name
                    ),
                )
            }
            // Only legal inside `def` bodies, which nothing renders yet.
            Node::Children { .. } => unreachable!("`children` parses only inside `def` bodies"),
        }
        Ok(())
    }

    /// SPEC §10.2: lists (index = position) and maps (name = value, index =
    /// key, insertion order); `empty` on empty-or-null; anything else errors.
    fn for_loop(&mut self, f: &ForLoop, depth: usize) -> Result<()> {
        match self.eval(&f.iter)? {
            Value::List(items) if !items.is_empty() => {
                for (i, item) in items.into_iter().enumerate() {
                    self.iteration(f, depth, item, Value::Number(i as f64))?;
                }
                Ok(())
            }
            Value::Map(entries) if !entries.is_empty() => {
                for (key, value) in entries {
                    self.iteration(f, depth, value, Value::Str(key))?;
                }
                Ok(())
            }
            Value::Null | Value::List(_) | Value::Map(_) => {
                if let Some(empty) = &f.empty {
                    self.block(empty, depth)?;
                }
                Ok(())
            }
            Value::Str(_) => err(
                f.iter.line,
                f.iter.col,
                "`for` cannot iterate a string — strings are not character sequences in fhtml",
            ),
            other => err(
                f.iter.line,
                f.iter.col,
                format!(
                    "`for` cannot iterate {} — it takes a list or map",
                    other.describe()
                ),
            ),
        }
    }

    /// One loop pass: bind var (and index), render, unbind. Loop variables
    /// shadow outer names, never `ctx` (the parser rejects that binding).
    fn iteration(&mut self, f: &ForLoop, depth: usize, item: Value, index: Value) -> Result<()> {
        self.scope.push(f.var.as_str(), item);
        if let Some(idx) = &f.index {
            self.scope.push(idx.as_str(), index);
        }
        let result = self.block(&f.body, depth);
        if f.index.is_some() {
            self.scope.pop();
        }
        self.scope.pop();
        result
    }

    fn element(&mut self, el: &Element, depth: usize) -> Result<()> {
        // A `>` chain renders glued on one line: opens (each followed by its own
        // inline text), then either the innermost's children as a block, or the
        // closings immediately (SPEC §4.6, §5).
        let mut opens = String::new();
        let mut closings = String::new();
        let mut cur = el;
        loop {
            opens.push_str(&self.open_tag(cur)?);
            if let Some(text) = &cur.text {
                opens.push_str(&self.text(text, false)?);
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

        let ind = if self.mode == Mode::Pretty {
            "  ".repeat(depth)
        } else {
            String::new()
        };
        let nl = if self.mode == Mode::Pretty { "\n" } else { "" };

        if inner.children.is_empty() {
            self.out.push_str(&format!("{ind}{opens}{closings}{nl}"));
        } else {
            self.out.push_str(&format!("{ind}{opens}{nl}"));
            for child in &inner.children {
                self.node(child, depth + 1)?;
            }
            self.out.push_str(&format!("{ind}{closings}{nl}"));
        }
        Ok(())
    }
}
