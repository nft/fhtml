//! `--target=js`: compiles an fhtml AST to a self-contained ES module
//! exporting `(data, ctx = {}) => string`, with semantics identical to the
//! Rust renderer (`emit.rs`) — the parity harness in `tests/jsgen.rs` holds
//! the two to byte equality. A tiny runtime prelude is embedded in every
//! module: no imports, no npm dependency.
//!
//! Loop variables compile to real JS bindings (prefixed `v_` so template
//! names can never collide with JS reserved words or the prelude): name
//! resolution is static, which matches the renderer's `Scope` because loop
//! shadowing is purely lexical. Every expression site is wrapped in `$at`,
//! which stamps render errors with the fhtml source line/column in the same
//! `line:col: error: …` format the Rust side uses.

use crate::emit::Mode;
use crate::expr::{BinOp, Expr};
use crate::parser::{
    is_void, AttrValue, ClassItem, Element, ForLoop, IfChain, Node, TextPart, TplExpr,
};

/// The runtime prelude. Helper contract mirrors `expr`/`emit`:
/// `$t` truthiness, `$eq` deep equality, `$s` stringify (throws on
/// list/map), `$fn` number formatting (shortest round-trip decimal, no
/// exponent, `-0` → `0`), `$add`/`$ar`/`$neg` arithmetic with the same error
/// messages, `$f`/`$ix`/`$d` null-safe access, `$it` for-iteration,
/// `$e`/`$ea` escaping, `$ci`/`$cj` class-list assembly, `$at` positions.
const PRELUDE: &str = r#"const $m = v => v !== null && typeof v === "object" && !Array.isArray(v);
const $desc = v => v == null ? "null" : Array.isArray(v) ? "a list" : typeof v === "object" ? "a map" : "a " + typeof v;
const $t = v => !(v == null || v === false || v === 0 || v === "" || (Array.isArray(v) && v.length === 0) || ($m(v) && Object.keys(v).length === 0));
const $fn = n => {
  if (n === 0) return "0";
  const s = String(n);
  const m = s.match(/^(-?)(\d+)(?:\.(\d+))?e([+-]\d+)$/);
  if (!m) return s;
  const [, sign, int, frac = "", exp] = m;
  const digits = int + frac;
  const point = int.length + Number(exp);
  if (point <= 0) return sign + "0." + "0".repeat(-point) + digits;
  if (point >= digits.length) return sign + digits + "0".repeat(point - digits.length);
  return sign + digits.slice(0, point) + "." + digits.slice(point);
};
const $s = v => {
  if (v == null) return "";
  if (typeof v === "boolean") return v ? "true" : "false";
  if (typeof v === "number") return $fn(v);
  if (typeof v === "string") return v;
  throw new Error("cannot interpolate " + $desc(v) + " — interpolation takes scalars (string, number, boolean, null)");
};
const $co = v => {
  if (Array.isArray(v) || $m(v)) throw new Error("cannot use " + $desc(v) + " with `+`");
  return $s(v);
};
const $fin = (op, n) => {
  if (!Number.isFinite(n)) throw new Error("`" + op + "` produced a non-finite number");
  return n;
};
const $add = (a, b) => {
  if (typeof a === "number" && typeof b === "number") return $fin("+", a + b);
  if (typeof a === "string") return a + $co(b);
  if (typeof b === "string") return $co(a) + b;
  throw new Error("`+` requires two numbers or a string operand, got " + $desc(a) + " and " + $desc(b));
};
const $ar = (op, a, b) => {
  if (typeof a !== "number" || typeof b !== "number")
    throw new Error("`" + op + "` requires numbers, got " + $desc(a) + " and " + $desc(b));
  switch (op) {
    case "<": return a < b;
    case "<=": return a <= b;
    case ">": return a > b;
    case ">=": return a >= b;
    case "-": return $fin(op, a - b);
    case "*": return $fin(op, a * b);
    case "/":
      if (b === 0) throw new Error("division by zero");
      return $fin(op, a / b);
    default:
      if (b === 0) throw new Error("modulo by zero");
      return $fin(op, a % b);
  }
};
const $neg = v => {
  if (typeof v !== "number") throw new Error("unary `-` requires a number, got " + $desc(v));
  return -v;
};
const $eq = (a, b) => {
  a = a ?? null; b = b ?? null;
  if (a === null || typeof a !== "object") return a === b;
  if (Array.isArray(a)) return Array.isArray(b) && a.length === b.length && a.every((x, i) => $eq(x, b[i]));
  if (!$m(b)) return false;
  const ka = Object.keys(a), kb = Object.keys(b);
  return ka.length === kb.length && ka.every(k => Object.hasOwn(b, k) && $eq(a[k], b[k]));
};
const $and = (a, b) => ($t(a) ? b() : a);
const $or = (a, b) => ($t(a) ? a : b());
const $f = (v, k) => ($m(v) && Object.hasOwn(v, k) ? v[k] ?? null : null);
const $ix = (v, i) => {
  if (Array.isArray(v) && typeof i === "number")
    return Number.isInteger(i) && i >= 0 && i < v.length ? v[i] ?? null : null;
  if ($m(v) && typeof i === "string") return Object.hasOwn(v, i) ? v[i] ?? null : null;
  return null;
};
const $it = v => {
  if (v == null) return [];
  if (Array.isArray(v)) return v.map((x, i) => [x ?? null, i]);
  if ($m(v)) return Object.entries(v).map(([k, x]) => [x ?? null, k]);
  if (typeof v === "string")
    throw new Error("`for` cannot iterate a string — strings are not character sequences in fhtml");
  throw new Error("`for` cannot iterate " + $desc(v) + " — it takes a list or map");
};
const $e = s => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
const $ea = s => $e(s).replace(/"/g, "&quot;");
const $ci = v => $s(v).split(/\s+/).filter(Boolean).map($ea);
const $cj = names => (names.length ? ' class="' + names.join(" ") + '"' : "");
const $at = (l, c, f) => {
  try { return f(); } catch (e) {
    if (e && e.fh) throw e;
    const n = new Error(l + ":" + c + ": error: " + (e && e.message ? e.message : e));
    n.fh = true;
    throw n;
  }
};
"#;

/// Generates the complete ES module for a parsed file. The output mode is
/// baked in at build time, exactly like the native renderer's `Mode`.
pub fn generate(nodes: &[Node], mode: Mode) -> String {
    let mut g = G {
        js: String::new(),
        buf: String::new(),
        code_ind: 1,
        bound: Vec::new(),
        mode,
        counter: 0,
    };
    g.block(nodes, 0);
    g.flush();
    let mut out = String::new();
    out.push_str("// generated by fhtml --target=js — do not edit\n");
    out.push_str(PRELUDE);
    out.push_str("export default function render(data, ctx = {}) {\n");
    out.push_str("  const D = data ?? null, C = ctx ?? null;\n");
    out.push_str("  let o = \"\";\n");
    out.push_str(&g.js);
    out.push_str("  return o;\n}\n");
    out
}

/// A JS double-quoted string literal.
fn js_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            // Legal in strings since ES2019, but escape for safety.
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// JS number literal from an fhtml literal. Rust `Display` output (full
/// decimal, no exponent) parses in JS back to the identical double.
fn js_num(n: f64) -> String {
    if n == 0.0 && n.is_sign_negative() {
        "-0".to_string()
    } else {
        n.to_string()
    }
}

/// Compiles an expression to JS against the statically-known loop bindings.
fn expr_js(e: &Expr, bound: &[String]) -> String {
    match e {
        Expr::Null => "null".to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::Number(n) => js_num(*n),
        Expr::Str(s) => js_str(s),
        Expr::Name(n) => {
            if n == "ctx" {
                "C".to_string()
            } else if bound.iter().any(|b| b == n) {
                format!("v_{n}")
            } else {
                format!("$f(D, {})", js_str(n))
            }
        }
        Expr::Field(base, name) => format!("$f({}, {})", expr_js(base, bound), js_str(name)),
        Expr::Index(base, idx) => format!("$ix({}, {})", expr_js(base, bound), expr_js(idx, bound)),
        Expr::Not(x) => format!("!$t({})", expr_js(x, bound)),
        Expr::Neg(x) => format!("$neg({})", expr_js(x, bound)),
        Expr::And(a, b) => format!("$and({}, () => {})", expr_js(a, bound), expr_js(b, bound)),
        Expr::Or(a, b) => format!("$or({}, () => {})", expr_js(a, bound), expr_js(b, bound)),
        Expr::Ternary(c, t, f) => format!(
            "($t({}) ? {} : {})",
            expr_js(c, bound),
            expr_js(t, bound),
            expr_js(f, bound)
        ),
        Expr::Binary(op, a, b) => {
            let (a, b) = (expr_js(a, bound), expr_js(b, bound));
            match op {
                BinOp::Eq => format!("$eq({a}, {b})"),
                BinOp::Ne => format!("!$eq({a}, {b})"),
                BinOp::Add => format!("$add({a}, {b})"),
                BinOp::Lt => format!("$ar(\"<\", {a}, {b})"),
                BinOp::Le => format!("$ar(\"<=\", {a}, {b})"),
                BinOp::Gt => format!("$ar(\">\", {a}, {b})"),
                BinOp::Ge => format!("$ar(\">=\", {a}, {b})"),
                BinOp::Sub => format!("$ar(\"-\", {a}, {b})"),
                BinOp::Mul => format!("$ar(\"*\", {a}, {b})"),
                BinOp::Div => format!("$ar(\"/\", {a}, {b})"),
                BinOp::Mod => format!("$ar(\"%\", {a}, {b})"),
            }
        }
    }
}

/// HTML entity escaping for static content, identical to `emit::esc`.
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

struct G {
    /// Generated function-body statements.
    js: String,
    /// Pending static output text, flushed as one `o += "…";`.
    buf: String,
    /// Code indentation level (readability of the module only).
    code_ind: usize,
    /// Loop variables in scope, innermost last.
    bound: Vec<String>,
    mode: Mode,
    counter: usize,
}

impl G {
    fn ind(&self, depth: usize) -> String {
        if self.mode == Mode::Pretty {
            "  ".repeat(depth)
        } else {
            String::new()
        }
    }

    fn nl(&self) -> &'static str {
        if self.mode == Mode::Pretty {
            "\n"
        } else {
            ""
        }
    }

    fn flush(&mut self) {
        if !self.buf.is_empty() {
            let lit = js_str(&self.buf);
            self.buf.clear();
            self.stmt(&format!("o += {lit};"));
        }
    }

    fn stmt(&mut self, s: &str) {
        self.js
            .push_str(&format!("{}{s}\n", "  ".repeat(self.code_ind)));
    }

    /// `o += <expr>;` after flushing static text.
    fn dynamic(&mut self, expr: String) {
        self.flush();
        self.stmt(&format!("o += {expr};"));
    }

    /// `$at(line, col, () => <js>)` — position-stamped evaluation.
    fn at(&self, t: &TplExpr, inner: String) -> String {
        format!("$at({}, {}, () => {inner})", t.line, t.col)
    }

    /// Evaluate + stringify with position.
    fn eval_string(&self, t: &TplExpr) -> String {
        self.at(t, format!("$s({})", expr_js(&t.expr, &self.bound)))
    }

    fn block(&mut self, nodes: &[Node], depth: usize) {
        for node in nodes {
            self.node(node, depth);
        }
    }

    fn node(&mut self, node: &Node, depth: usize) {
        let ind = self.ind(depth);
        let nl = self.nl();
        match node {
            Node::Doctype => self.buf.push_str(&format!("{ind}<!DOCTYPE html>{nl}")),
            Node::Comment { emit: false, .. } => {}
            Node::Comment { lines, emit: true } => {
                if lines.len() == 1 {
                    self.buf
                        .push_str(&format!("{ind}<!-- {} -->{nl}", lines[0]));
                } else {
                    self.buf.push_str(&format!("{ind}<!-- {}{nl}", lines[0]));
                    for l in &lines[1..] {
                        self.buf.push_str(&format!("{ind}{l}{nl}"));
                    }
                    if self.mode == Mode::Min {
                        self.buf.push('\n');
                    }
                    self.buf.push_str(&format!("{ind}-->{nl}"));
                }
            }
            Node::Raw(lines) => {
                for (i, l) in lines.iter().enumerate() {
                    if self.mode == Mode::Min && i > 0 {
                        self.buf.push('\n');
                    }
                    if l.is_empty() {
                        self.buf.push_str(nl);
                    } else {
                        self.buf.push_str(&format!("{ind}{l}{nl}"));
                    }
                }
            }
            Node::TextBlock(lines) => {
                for (i, parts) in lines.iter().enumerate() {
                    if self.mode == Mode::Min && i > 0 {
                        self.buf.push('\n');
                    }
                    self.buf.push_str(&ind);
                    self.text(parts, false);
                    self.buf.push_str(nl);
                }
            }
            Node::Element(el) => self.element(el, depth),
            Node::If(chain) => self.if_chain(chain, depth),
            Node::For(f) => self.for_loop(f, depth),
        }
    }

    /// Text segments into the output; `quotes` selects attribute escaping.
    fn text(&mut self, parts: &[TextPart], quotes: bool) {
        for part in parts {
            match part {
                TextPart::Lit(l) => self.buf.push_str(&esc(l, quotes)),
                TextPart::Interp { expr, raw } => {
                    let s = self.eval_string(expr);
                    if *raw {
                        self.dynamic(s);
                    } else if quotes {
                        self.dynamic(format!("$ea({s})"));
                    } else {
                        self.dynamic(format!("$e({s})"));
                    }
                }
            }
        }
    }

    fn open_tag(&mut self, el: &Element) {
        self.buf.push_str(&format!("<{}", el.tag));
        if let Some(id) = &el.id {
            self.buf.push_str(&format!(" id=\"{}\"", esc(id, true)));
        }
        for (name, value) in &el.attrs {
            match value {
                AttrValue::Bool => self.buf.push_str(&format!(" {name}")),
                AttrValue::Str(parts) => {
                    self.buf.push_str(&format!(" {name}=\""));
                    self.text(parts, true);
                    self.buf.push('"');
                }
                AttrValue::Expr(t) => {
                    self.buf.push_str(&format!(" {name}=\""));
                    let s = self.eval_string(t);
                    self.dynamic(format!("$ea({s})"));
                    self.buf.push('"');
                }
            }
        }
        self.classes(el);
        self.buf.push('>');
    }

    /// The class attribute. All-literal lists bake in statically; any
    /// interpolation defers assembly (and empty-list omission) to `$cj`.
    fn classes(&mut self, el: &Element) {
        if el.classes.is_empty() {
            return;
        }
        let all_lit = el.classes.iter().all(|c| matches!(c, ClassItem::Lit(_)));
        if all_lit {
            let joined = el
                .classes
                .iter()
                .map(|c| match c {
                    ClassItem::Lit(s) => s.replace('"', "&quot;"),
                    ClassItem::Interp(_) => unreachable!(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            self.buf.push_str(&format!(" class=\"{joined}\""));
            return;
        }
        let items = el
            .classes
            .iter()
            .map(|c| match c {
                ClassItem::Lit(s) => js_str(&s.replace('"', "&quot;")),
                ClassItem::Interp(t) => {
                    // `$ci` stringifies inside `$at` so a list/map error
                    // carries this interpolation's position.
                    format!(
                        "...{}",
                        self.at(t, format!("$ci({})", expr_js(&t.expr, &self.bound)))
                    )
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.dynamic(format!("$cj([{items}])"));
    }

    fn if_chain(&mut self, chain: &IfChain, depth: usize) {
        self.flush();
        for (i, arm) in chain.arms.iter().enumerate() {
            let cond = self.at(&arm.cond, expr_js(&arm.cond.expr, &self.bound));
            let kw = if i == 0 { "if" } else { "} else if" };
            self.stmt(&format!("{kw} ($t({cond})) {{"));
            self.code_ind += 1;
            self.block(&arm.body, depth);
            self.flush();
            self.code_ind -= 1;
        }
        if let Some(else_body) = &chain.else_body {
            self.stmt("} else {");
            self.code_ind += 1;
            self.block(else_body, depth);
            self.flush();
            self.code_ind -= 1;
        }
        self.stmt("}");
    }

    fn for_loop(&mut self, f: &ForLoop, depth: usize) {
        self.flush();
        self.counter += 1;
        let es = format!("$es{}", self.counter);
        let iter = self.at(
            &f.iter,
            format!("$it({})", expr_js(&f.iter.expr, &self.bound)),
        );
        self.stmt("{");
        self.code_ind += 1;
        self.stmt(&format!("const {es} = {iter};"));
        if f.empty.is_some() {
            self.stmt(&format!("if ({es}.length) {{"));
            self.code_ind += 1;
        }
        let binding = match &f.index {
            Some(idx) => format!("[v_{}, v_{idx}]", f.var),
            None => format!("[v_{}]", f.var),
        };
        self.stmt(&format!("for (const {binding} of {es}) {{"));
        self.code_ind += 1;
        self.bound.push(f.var.clone());
        if let Some(idx) = &f.index {
            self.bound.push(idx.clone());
        }
        self.block(&f.body, depth);
        self.flush();
        if f.index.is_some() {
            self.bound.pop();
        }
        self.bound.pop();
        self.code_ind -= 1;
        self.stmt("}");
        if let Some(empty) = &f.empty {
            self.code_ind -= 1;
            self.stmt("} else {");
            self.code_ind += 1;
            self.block(empty, depth);
            self.flush();
            self.code_ind -= 1;
            self.stmt("}");
        }
        self.code_ind -= 1;
        self.stmt("}");
    }

    fn element(&mut self, el: &Element, depth: usize) {
        let ind = self.ind(depth);
        let nl = self.nl();
        self.buf.push_str(&ind);

        // Chain: opens (each with its inline text), then children or closings
        // (SPEC §4.6, §5) — same shape as the native renderer.
        let mut closings = String::new();
        let mut cur = el;
        loop {
            self.open_tag(cur);
            if let Some(text) = &cur.text {
                self.text(text, false);
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

        if inner.children.is_empty() {
            self.buf.push_str(&format!("{closings}{nl}"));
        } else {
            self.buf.push_str(nl);
            for child in &inner.children {
                self.node(child, depth + 1);
            }
            self.buf.push_str(&format!("{ind}{closings}{nl}"));
        }
    }
}
