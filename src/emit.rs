//! Renderer (SPEC §9–§11). Two modes producing the same element tree:
//! `Pretty` (2-space indented) and `Min` (no inter-tag whitespace).
//!
//! One code path serves both the static and template layers: `compile`
//! rejects template constructs and then renders with null data (a
//! literal-only tree evaluates nothing, so its output is byte-identical to
//! the static emitter and cannot error); `render` evaluates statements and
//! interpolation against caller data. Render errors carry the file position
//! of the interpolation or statement, in the same format as parse errors.

use std::collections::HashMap;

use crate::error::{err, Error, Result};
use crate::expr::{self, Scope, Value};
use crate::parser::{
    is_void, Arg, AttrValue, Call, ClassItem, Def, Document, Element, ForLoop, Node, TextPart,
    TplExpr,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Pretty,
    Min,
}

/// Component call-depth cap (SPEC §10.3): recursion is legal, runaway
/// recursion is a render error at the call site. `jsgen` bakes the same cap
/// (and message) into compiled modules.
pub(crate) const MAX_CALL_DEPTH: usize = 64;

/// Root of every component scope — components are closed over nothing
/// (SPEC §10.3), so inside a body any name that is not a parameter is `null`.
static NULL: Value = Value::Null;

/// Renders a document against `data` (the name-resolution root) and `ctx`
/// (the reserved context root, SPEC §9.4). Pass `Value::Null` for an empty
/// scope. All component calls are checked up front — unknown components,
/// unknown or missing arguments, and dropped blocks error before any output.
pub fn render_document<'a>(
    doc: &'a Document,
    mode: Mode,
    data: &'a Value,
    ctx: &'a Value,
) -> Result<String> {
    let defs = check_components(doc)?;
    let mut r = R {
        mode,
        scope: Scope::new(data),
        ctx,
        out: String::new(),
        defs,
        frames: Vec::new(),
    };
    r.block(&doc.body, 0)?;
    Ok(r.out)
}

/// A component with its lexical `children`-usage, precomputed once.
pub(crate) struct DefInfo<'a> {
    pub(crate) def: &'a Def,
    uses_children: bool,
}

/// Builds the component table and statically checks every call in the file
/// (SPEC §10.4) — body and def bodies alike, so a mistake errors even when
/// the call sits on a branch this render never takes. Shared with `jsgen`:
/// `--target=js` reports the same mistakes with the same messages, at
/// compile time.
pub(crate) fn check_components(doc: &Document) -> Result<HashMap<&str, DefInfo<'_>>> {
    let mut defs = HashMap::new();
    for def in &doc.defs {
        let mut uses_children = false;
        visit(&def.body, &mut |node| {
            if matches!(node, Node::Children { .. }) {
                uses_children = true;
            }
            Ok(())
        })?;
        // Duplicate names are a parse error, so plain insert.
        defs.insert(def.name.as_str(), DefInfo { def, uses_children });
    }
    let mut check = |node: &Node| {
        let Node::Call(c) = node else { return Ok(()) };
        let Some(info) = defs.get(c.name.as_str()) else {
            return err(
                c.line,
                1,
                format!(
                    "unknown component `+{}` — no `def {}(…)` in this file",
                    c.name, c.name
                ),
            );
        };
        for arg in &c.args {
            if !info.def.params.iter().any(|p| p.name == arg.name) {
                return err(
                    arg.line,
                    arg.col,
                    format!(
                        "unknown argument `{}` — `{}` has {}",
                        arg.name,
                        c.name,
                        describe_params(info.def)
                    ),
                );
            }
        }
        for p in &info.def.params {
            if p.default.is_none() && !c.args.iter().any(|a| a.name == p.name) {
                return err(
                    c.line,
                    1,
                    format!(
                        "missing argument `{}` — the parameter has no default in `def {}` (line {})",
                        p.name, c.name, info.def.line
                    ),
                );
            }
        }
        if !c.children.is_empty() && !info.uses_children {
            return err(
                c.line,
                1,
                format!(
                    "`{}` never uses `children`, so this block would be dropped (SPEC §10.4) — remove it, or add `children` to `def {}` (line {})",
                    c.name, c.name, info.def.line
                ),
            );
        }
        Ok(())
    };
    for def in &doc.defs {
        visit(&def.body, &mut check)?;
    }
    visit(&doc.body, &mut check)?;
    Ok(defs)
}

fn describe_params(def: &Def) -> String {
    if def.params.is_empty() {
        "no parameters".to_string()
    } else {
        let names: Vec<_> = def.params.iter().map(|p| format!("`{}`", p.name)).collect();
        format!("parameters {}", names.join(", "))
    }
}

/// Depth-first visit of every node in a tree, including statement bodies,
/// chain children, and the blocks passed to calls.
fn visit<'a>(nodes: &'a [Node], f: &mut impl FnMut(&'a Node) -> Result<()>) -> Result<()> {
    for node in nodes {
        f(node)?;
        match node {
            Node::Element(el) => {
                let mut cur = el;
                loop {
                    visit(&cur.children, f)?;
                    match &cur.chain {
                        Some(next) => cur = next,
                        None => break,
                    }
                }
            }
            Node::If(chain) => {
                for arm in &chain.arms {
                    visit(&arm.body, f)?;
                }
                if let Some(body) = &chain.else_body {
                    visit(body, f)?;
                }
            }
            Node::For(l) => {
                visit(&l.body, f)?;
                if let Some(empty) = &l.empty {
                    visit(empty, f)?;
                }
            }
            Node::Call(c) => visit(&c.children, f)?,
            Node::Children { .. }
            | Node::DefSite(_)
            | Node::TextBlock(_)
            | Node::Raw(_)
            | Node::Comment { .. }
            | Node::Doctype => {}
            Node::Include { .. } => {
                unreachable!("includes are resolved before rendering (SPEC §10.5)")
            }
        }
    }
    Ok(())
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
    defs: HashMap<&'a str, DefInfo<'a>>,
    /// One frame per component call in progress — the call depth. Each holds
    /// the caller's context, captured at call time, which `children` restores
    /// while it renders the caller's block (SPEC §10.3–§10.4).
    frames: Vec<Frame<'a>>,
}

/// The caller's context, saved across a component call.
struct Frame<'a> {
    scope: Scope<'a>,
    children: &'a [Node],
}

impl<'a> R<'a> {
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

    fn block(&mut self, nodes: &'a [Node], depth: usize) -> Result<()> {
        for node in nodes {
            self.node(node, depth)?;
        }
        Ok(())
    }

    fn node(&mut self, node: &'a Node, depth: usize) -> Result<()> {
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
            Node::Call(c) => self.call(c, depth)?,
            // Emits the caller's block in the caller's context (SPEC §10.3).
            // The parser guarantees this sits inside a `def` body, and def
            // bodies only render through `call`, so a frame always exists.
            Node::Children { .. } => {
                let mut frame = self.frames.pop().expect("`children` renders under a call");
                std::mem::swap(&mut self.scope, &mut frame.scope);
                let result = self.block(frame.children, depth);
                std::mem::swap(&mut self.scope, &mut frame.scope);
                self.frames.push(frame);
                result?;
            }
            // A definition emits nothing where it stands (SPEC §10.3).
            Node::DefSite(_) => {}
            Node::Include { .. } => {
                unreachable!("includes are resolved before rendering (SPEC §10.5)")
            }
        }
        Ok(())
    }

    /// Expands a component call (SPEC §10.3–§10.4): arguments and defaults
    /// evaluate in the caller's scope, the body renders in a fresh scope
    /// holding only the parameters (plus the unshadowable `ctx`), and the
    /// caller's context is framed for `children`.
    fn call(&mut self, c: &'a Call, depth: usize) -> Result<()> {
        if self.frames.len() >= MAX_CALL_DEPTH {
            return err(
                c.line,
                1,
                format!(
                    "`+{}` exceeds the component call depth cap of {MAX_CALL_DEPTH} (SPEC §10.3) — recursion needs a base case",
                    c.name
                ),
            );
        }
        let def = self.defs[c.name.as_str()].def; // resolved in check_components
        let mut scope = Scope::new(&NULL);
        for p in &def.params {
            let value = match c.args.iter().find(|a| a.name == p.name) {
                Some(arg) => self.arg_value(arg)?,
                // Presence was checked up front: no argument means a default
                // exists. Defaults are expressions, evaluated at each call in
                // the caller's scope (SPEC §10.3).
                None => self.eval(p.default.as_ref().expect("checked"))?,
            };
            scope.push(p.name.as_str(), value);
        }
        let caller = std::mem::replace(&mut self.scope, scope);
        self.frames.push(Frame {
            scope: caller,
            children: &c.children,
        });
        let result = self.block(&def.body, depth);
        let frame = self.frames.pop().expect("pushed above");
        self.scope = frame.scope;
        result
    }

    /// An argument's value (SPEC §10.4): bare = `true`; unquoted = the
    /// expression's value; quoted = the string, interpolations stringified —
    /// no entity escaping here, the value is data, not output.
    fn arg_value(&self, arg: &Arg) -> Result<Value> {
        match &arg.value {
            AttrValue::Bool => Ok(Value::Bool(true)),
            AttrValue::Expr(t) => self.eval(t),
            AttrValue::Str(parts) => {
                let mut s = String::new();
                for part in parts {
                    match part {
                        TextPart::Lit(l) => s.push_str(l),
                        // Raw `{!…}` cannot parse in argument strings.
                        TextPart::Interp { expr, .. } => s.push_str(&self.eval_string(expr)?),
                    }
                }
                Ok(Value::Str(s))
            }
        }
    }

    /// SPEC §10.2: lists (index = position) and maps (name = value, index =
    /// key, insertion order); `empty` on empty-or-null; anything else errors.
    fn for_loop(&mut self, f: &'a ForLoop, depth: usize) -> Result<()> {
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
    fn iteration(&mut self, f: &'a ForLoop, depth: usize, item: Value, index: Value) -> Result<()> {
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

    fn element(&mut self, el: &'a Element, depth: usize) -> Result<()> {
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
