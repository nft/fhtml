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
//!
//! Components (SPEC §10.3–§10.4): each `def` compiles to a nested function
//! `(v_param…, children, $i) => void` appending to the shared output, with
//! parameters as real bindings — a closed scope falls out of lexical
//! resolution (unbound names inside a body compile to `null`). `children`
//! is a thunk built at the call site, closing over the caller's JS scope,
//! which is exactly the renderer's caller-frame semantics. Because a body
//! renders at its call site's depth — dynamic under recursion — Pretty-mode
//! indentation flows through the `$i` parameter at runtime; static text
//! outside components is unaffected. The call-depth cap is a shared `$d`
//! counter with the renderer's exact message and call-site position.

use std::collections::HashMap;

use crate::emit::{check_components, Mode, MAX_CALL_DEPTH};
use crate::error::Result;
use crate::expr::{BinOp, Expr};
use crate::parser::{
    is_void, AttrValue, Call, ClassItem, Def, Document, Element, ForLoop, IfChain, Node, Param,
    TextPart, TplExpr,
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
const $ci = v => (typeof v === "boolean" || !$t(v)) ? [] : $s(v).split(/\s+/).filter(Boolean).map($ea);
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
/// Component mistakes (unknown component, unknown/missing argument, dropped
/// block) are compile errors here — same checks, same messages as the
/// renderer, which reports them at render time.
pub fn generate(doc: &Document, mode: Mode) -> Result<String> {
    let table = check_components(doc)?;
    let defs: HashMap<&str, &Def> = table.iter().map(|(name, info)| (*name, info.def)).collect();
    let mut g = G {
        js: String::new(),
        buf: Vec::new(),
        code_ind: 1,
        bound: Vec::new(),
        mode,
        counter: 0,
        defs,
        in_def: false,
        dyn_ind: false,
    };
    for def in &doc.defs {
        g.def_fn(def);
    }
    g.block(&doc.body, 0);
    g.flush();
    let mut out = String::new();
    out.push_str("// generated by fhtml --target=js — do not edit\n");
    out.push_str(PRELUDE);
    out.push_str("export default function render(data, ctx = {}) {\n");
    out.push_str("  const D = data ?? null, C = ctx ?? null;\n");
    out.push_str("  let o = \"\";\n");
    if !doc.defs.is_empty() {
        out.push_str("  let $d = 0;\n");
    }
    out.push_str(&g.js);
    out.push_str("  return o;\n}\n");
    Ok(out)
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

/// Compiles an expression to JS against the statically-known bindings (loop
/// variables, and parameters inside a `def` body). `closed` is the component
/// case (SPEC §10.3): bodies are closed over nothing, so an unbound name is
/// `null` instead of a data-root lookup.
fn expr_js(e: &Expr, bound: &[String], closed: bool) -> String {
    match e {
        Expr::Null => "null".to_string(),
        Expr::Bool(b) => b.to_string(),
        Expr::Number(n) => js_num(*n),
        Expr::Name(n) => {
            if n == "ctx" {
                "C".to_string()
            } else if bound.iter().any(|b| b == n) {
                format!("v_{n}")
            } else if closed {
                "null".to_string()
            } else {
                format!("$f(D, {})", js_str(n))
            }
        }
        Expr::Str(s) => js_str(s),
        Expr::Field(base, name) => {
            format!("$f({}, {})", expr_js(base, bound, closed), js_str(name))
        }
        Expr::Index(base, idx) => format!(
            "$ix({}, {})",
            expr_js(base, bound, closed),
            expr_js(idx, bound, closed)
        ),
        Expr::Not(x) => format!("!$t({})", expr_js(x, bound, closed)),
        Expr::Neg(x) => format!("$neg({})", expr_js(x, bound, closed)),
        Expr::And(a, b) => format!(
            "$and({}, () => {})",
            expr_js(a, bound, closed),
            expr_js(b, bound, closed)
        ),
        Expr::Or(a, b) => format!(
            "$or({}, () => {})",
            expr_js(a, bound, closed),
            expr_js(b, bound, closed)
        ),
        Expr::Ternary(c, t, f) => format!(
            "($t({}) ? {} : {})",
            expr_js(c, bound, closed),
            expr_js(t, bound, closed),
            expr_js(f, bound, closed)
        ),
        Expr::Binary(op, a, b) => {
            let (a, b) = (expr_js(a, bound, closed), expr_js(b, bound, closed));
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

/// A piece of pending static output. Outside components everything is `Lit`;
/// inside a `def` body or a `children` thunk, Pretty-mode line starts are
/// `Ind` — the runtime base indent `$i` (relative depth follows as a `Lit`
/// of spaces, merged into the next chunk).
enum Chunk {
    Lit(String),
    Ind,
}

struct G<'a> {
    /// Generated function-body statements.
    js: String,
    /// Pending static output, flushed as one `o += …;` concatenation.
    buf: Vec<Chunk>,
    /// Code indentation level (readability of the module only).
    code_ind: usize,
    /// Names with a JS binding in scope, innermost last: loop variables,
    /// plus the parameters inside a `def` body.
    bound: Vec<String>,
    mode: Mode,
    counter: usize,
    /// Component table (validated by `check_components`), for call sites.
    defs: HashMap<&'a str, &'a Def>,
    /// Inside a `def` body: unbound names are `null` (closed scope).
    in_def: bool,
    /// Line starts are dynamic (`$i`-based): `def` bodies and `children`
    /// thunks, whose indent is decided at runtime. Pretty mode only.
    dyn_ind: bool,
}

impl<'a> G<'a> {
    fn nl(&self) -> &'static str {
        if self.mode == Mode::Pretty {
            "\n"
        } else {
            ""
        }
    }

    /// Appends static text, merging into the last literal chunk.
    fn lit(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if let Some(Chunk::Lit(l)) = self.buf.last_mut() {
            l.push_str(s);
        } else {
            self.buf.push(Chunk::Lit(s.to_string()));
        }
    }

    /// A line-start indent at `depth`. Static outside components; inside
    /// them (Pretty), the runtime `$i` plus the relative depth.
    fn push_ind(&mut self, depth: usize) {
        if self.mode != Mode::Pretty {
            return;
        }
        if self.dyn_ind {
            self.buf.push(Chunk::Ind);
        }
        if depth > 0 {
            self.lit(&"  ".repeat(depth));
        }
    }

    /// The indent value passed to a `def` call or `children` thunk at
    /// `depth` — the callee's runtime base indent. Pretty mode only.
    fn ind_arg(&self, depth: usize) -> String {
        let rel = "  ".repeat(depth);
        if self.dyn_ind {
            if depth == 0 {
                "$i".to_string()
            } else {
                format!("$i + {}", js_str(&rel))
            }
        } else {
            js_str(&rel)
        }
    }

    fn flush(&mut self) {
        if self.buf.is_empty() {
            return;
        }
        let parts: Vec<String> = self
            .buf
            .drain(..)
            .map(|c| match c {
                Chunk::Lit(s) => js_str(&s),
                Chunk::Ind => "$i".to_string(),
            })
            .collect();
        let expr = parts.join(" + ");
        self.stmt(&format!("o += {expr};"));
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

    fn expr(&self, t: &TplExpr) -> String {
        expr_js(&t.expr, &self.bound, self.in_def)
    }

    /// Evaluate + stringify with position.
    fn eval_string(&self, t: &TplExpr) -> String {
        self.at(t, format!("$s({})", self.expr(t)))
    }

    fn block(&mut self, nodes: &[Node], depth: usize) {
        for node in nodes {
            self.node(node, depth);
        }
    }

    fn node(&mut self, node: &Node, depth: usize) {
        let nl = self.nl();
        match node {
            Node::Doctype => {
                self.push_ind(depth);
                self.lit("<!DOCTYPE html>");
                self.lit(nl);
            }
            Node::Comment { emit: false, .. } => {}
            Node::Comment { lines, emit: true } => {
                if lines.len() == 1 {
                    self.push_ind(depth);
                    self.lit(&format!("<!-- {} -->{nl}", lines[0]));
                } else {
                    self.push_ind(depth);
                    self.lit(&format!("<!-- {}{nl}", lines[0]));
                    for l in &lines[1..] {
                        self.push_ind(depth);
                        self.lit(&format!("{l}{nl}"));
                    }
                    if self.mode == Mode::Min {
                        self.lit("\n");
                    }
                    self.push_ind(depth);
                    self.lit(&format!("-->{nl}"));
                }
            }
            Node::Raw(lines) => {
                for (i, l) in lines.iter().enumerate() {
                    if self.mode == Mode::Min && i > 0 {
                        self.lit("\n");
                    }
                    if l.is_empty() {
                        self.lit(nl);
                    } else {
                        self.push_ind(depth);
                        self.lit(&format!("{l}{nl}"));
                    }
                }
            }
            Node::TextBlock(lines) => {
                for (i, parts) in lines.iter().enumerate() {
                    if self.mode == Mode::Min && i > 0 {
                        self.lit("\n");
                    }
                    self.push_ind(depth);
                    self.text(parts, false);
                    self.lit(nl);
                }
            }
            Node::Element(el) => self.element(el, depth),
            Node::If(chain) => self.if_chain(chain, depth),
            Node::For(f) => self.for_loop(f, depth),
            Node::Call(c) => self.call(c, depth),
            // Emits the caller's block: the thunk built at the call site,
            // closing over the caller's JS scope (SPEC §10.3). It renders
            // at this statement's depth, like any statement body.
            Node::Children { .. } => {
                self.flush();
                if self.mode == Mode::Pretty {
                    let ind = self.ind_arg(depth);
                    self.stmt(&format!("children({ind});"));
                } else {
                    self.stmt("children();");
                }
            }
            // A definition emits nothing where it stands (SPEC §10.3);
            // `generate` compiled it to a function up front.
            Node::DefSite(_) => {}
            Node::Include { .. } => {
                unreachable!("includes are resolved before codegen (SPEC §10.5)")
            }
        }
    }

    /// One `def` as a nested function appending to the shared output. The
    /// body compiles with only the parameters bound and unbound names
    /// `null` — the closed scope of SPEC §10.3 via lexical resolution.
    /// Mutual recursion works because bodies only run after every `const`
    /// initializes.
    fn def_fn(&mut self, def: &Def) {
        let mut params: Vec<String> = def.params.iter().map(|p| format!("v_{}", p.name)).collect();
        params.push("children".to_string());
        if self.mode == Mode::Pretty {
            params.push("$i".to_string());
        }
        self.stmt(&format!(
            "const c_{} = ({}) => {{",
            def.name,
            params.join(", ")
        ));
        self.code_ind += 1;
        self.bound = def.params.iter().map(|p| p.name.clone()).collect();
        self.in_def = true;
        self.dyn_ind = self.mode == Mode::Pretty;
        self.block(&def.body, 0);
        self.flush();
        self.dyn_ind = false;
        self.in_def = false;
        self.bound.clear();
        self.code_ind -= 1;
        self.stmt("};");
    }

    /// A component call (SPEC §10.3–§10.4), mirroring the renderer's order:
    /// depth check first, then arguments and defaults evaluated in the
    /// caller's scope in parameter order (JS argument evaluation is
    /// left-to-right), then the body at this call's depth.
    fn call(&mut self, c: &Call, depth: usize) {
        self.flush();
        let def = self.defs[c.name.as_str()];
        let msg = js_str(&format!(
            "`+{}` exceeds the component call depth cap of {MAX_CALL_DEPTH} (SPEC §10.3) — recursion needs a base case",
            c.name
        ));
        self.stmt(&format!(
            "if ($d >= {MAX_CALL_DEPTH}) $at({}, 1, () => {{ throw new Error({msg}); }});",
            c.line
        ));
        self.stmt("$d++;");
        let mut args: Vec<String> = def.params.iter().map(|p| self.arg_js(c, p)).collect();
        let ind = self.ind_arg(depth);
        if c.children.is_empty() {
            args.push("() => {}".to_string());
            if self.mode == Mode::Pretty {
                args.push(ind);
            }
            self.stmt(&format!("c_{}({});", c.name, args.join(", ")));
        } else {
            let thunk_param = if self.mode == Mode::Pretty { "$i" } else { "" };
            let lead = args.iter().fold(String::new(), |s, a| s + a + ", ");
            self.stmt(&format!("c_{}({lead}({thunk_param}) => {{", c.name));
            self.code_ind += 1;
            let saved = self.dyn_ind;
            self.dyn_ind = self.mode == Mode::Pretty;
            self.block(&c.children, 0);
            self.flush();
            self.dyn_ind = saved;
            self.code_ind -= 1;
            if self.mode == Mode::Pretty {
                self.stmt(&format!("}}, {ind});"));
            } else {
                self.stmt("});");
            }
        }
        self.stmt("$d--;");
    }

    /// One parameter's value at a call site (SPEC §10.4): the argument if
    /// given (bare = `true`, quoted = string built without escaping — the
    /// value is data, not output — unquoted = the expression), else the
    /// default. Either way an expression in the caller's scope, `$at` the
    /// argument's own position.
    fn arg_js(&self, c: &Call, p: &Param) -> String {
        match c.args.iter().find(|a| a.name == p.name) {
            Some(arg) => match &arg.value {
                AttrValue::Bool => "true".to_string(),
                AttrValue::Expr(t) => self.at(t, self.expr(t)),
                AttrValue::Str(parts) => {
                    if parts.is_empty() {
                        return js_str("");
                    }
                    let parts: Vec<String> = parts
                        .iter()
                        .map(|part| match part {
                            TextPart::Lit(l) => js_str(l),
                            // Raw `{!…}` cannot parse in argument strings.
                            TextPart::Interp { expr, .. } => self.eval_string(expr),
                        })
                        .collect();
                    parts.join(" + ")
                }
            },
            // Presence was checked in `check_components`: no argument means
            // a default exists (SPEC §10.3).
            None => {
                let d = p.default.as_ref().expect("checked");
                self.at(d, self.expr(d))
            }
        }
    }

    /// Text segments into the output; `quotes` selects attribute escaping.
    fn text(&mut self, parts: &[TextPart], quotes: bool) {
        for part in parts {
            match part {
                TextPart::Lit(l) => self.lit(&esc(l, quotes)),
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
        self.lit(&format!("<{}", el.tag));
        if let Some(id) = &el.id {
            self.lit(&format!(" id=\"{}\"", esc(id, true)));
        }
        for (name, value) in &el.attrs {
            match value {
                AttrValue::Bool => self.lit(&format!(" {name}")),
                AttrValue::Str(parts) => {
                    self.lit(&format!(" {name}=\""));
                    self.text(parts, true);
                    self.lit("\"");
                }
                AttrValue::Expr(t) => {
                    self.lit(&format!(" {name}=\""));
                    let s = self.eval_string(t);
                    self.dynamic(format!("$ea({s})"));
                    self.lit("\"");
                }
            }
        }
        self.classes(el);
        self.lit(">");
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
            self.lit(&format!(" class=\"{joined}\""));
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
                    format!("...{}", self.at(t, format!("$ci({})", self.expr(t))))
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        self.dynamic(format!("$cj([{items}])"));
    }

    fn if_chain(&mut self, chain: &IfChain, depth: usize) {
        self.flush();
        for (i, arm) in chain.arms.iter().enumerate() {
            let cond = self.at(&arm.cond, self.expr(&arm.cond));
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
        let iter = self.at(&f.iter, format!("$it({})", self.expr(&f.iter)));
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
        let nl = self.nl();
        self.push_ind(depth);

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

        if let Some(body) = &inner.raw_body {
            // Raw-text body (SPEC §6.3): verbatim static bytes, byte-identical
            // in both modes — the tags hug the content, never reindented.
            self.lit(&body.join("\n"));
            self.lit(&format!("{closings}{nl}"));
        } else if inner.children.is_empty() {
            self.lit(&format!("{closings}{nl}"));
        } else {
            self.lit(nl);
            for child in &inner.children {
                self.node(child, depth + 1);
            }
            self.push_ind(depth);
            self.lit(&format!("{closings}{nl}"));
        }
    }
}
