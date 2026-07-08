//! Parser for the fhtml markup layer (SPEC §1–§8) and the template layer
//! (SPEC §9 interpolation, §10.1–§10.2 statements, §10.3–§10.4 components).
//!
//! `include` (SPEC §10.5) is recognized and rejected with a clear "not
//! implemented" error so the syntax space stays reserved. Parsing with
//! `templates: false` enforces static-only (SPEC §9.2): any template construct is an
//! error.

use crate::error::{err, Result};
use crate::expr::{self, Expr};

pub const RESERVED: [&str; 8] = [
    "if", "elif", "else", "for", "empty", "def", "children", "include",
];

pub const VOID: [&str; 13] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

pub fn is_void(tag: &str) -> bool {
    VOID.contains(&tag)
}

/// An expression embedded in the template layer: the parsed AST plus the
/// trimmed source text between the braces / after the keyword (`fhtml fmt`
/// reprints this text — it never re-serializes the AST) and the expression's
/// 1-based position (line; column of the `{` or of the expression start,
/// counted within the logical line's content).
#[derive(Debug, Clone)]
pub struct TplExpr {
    pub src: String,
    pub expr: Expr,
    pub line: usize,
    pub col: usize,
}

/// One segment of interpolatable text (SPEC §9.1): inline `"…"` text, `|`
/// block lines, and quoted attribute values are sequences of these.
#[derive(Debug, Clone)]
pub enum TextPart {
    Lit(String),
    /// `{expr}` (escaped on output) or `{!expr}` (raw; content positions only).
    Interp {
        expr: TplExpr,
        raw: bool,
    },
}

/// Builds the segment list for literal-only text (used by the converter).
pub fn lit_parts(s: &str) -> Vec<TextPart> {
    if s.is_empty() {
        Vec::new()
    } else {
        vec![TextPart::Lit(s.to_string())]
    }
}

/// One entry of an element's class list: a verbatim class token, or a
/// `{expr}` whose result splits on whitespace into class names at render
/// time (SPEC §9.2; always attribute-escaped, `{!…}` forbidden here).
#[derive(Debug, Clone)]
pub enum ClassItem {
    Lit(String),
    Interp(TplExpr),
}

/// A parsed file: the component definitions (top-level only, SPEC §10.3) and
/// the markup that renders — a `def` emits nothing at its definition site.
#[derive(Debug)]
pub struct Document {
    pub defs: Vec<Def>,
    pub body: Vec<Node>,
}

/// One `def name(param param=default …)` component (SPEC §10.3). Definition
/// order doesn't matter: calls may reference a `def` that appears later, so
/// name resolution happens at render time, not here.
#[derive(Debug)]
pub struct Def {
    pub name: String,
    pub params: Vec<Param>,
    pub body: Vec<Node>,
    pub line: usize,
}

#[derive(Debug)]
pub struct Param {
    pub name: String,
    /// Default value — an expression (SPEC §10.3), evaluated at each call.
    pub default: Option<TplExpr>,
}

/// One `+name(args)` component call (SPEC §10.4). Arguments are named-only;
/// whether each name matches a parameter is checked at render time (the
/// `def` may appear later in the file).
#[derive(Debug)]
pub struct Call {
    pub name: String,
    pub args: Vec<Arg>,
    /// The indented block — the component's `children`, evaluated in the
    /// caller's scope.
    pub children: Vec<Node>,
    pub line: usize,
}

/// A call argument. The value reuses the attribute shapes: `Bool` for a bare
/// name (= `true`), `Str` for a quoted string with interpolation, `Expr` for
/// an unquoted expression (SPEC §10.4 — never a coerced string).
#[derive(Debug)]
pub struct Arg {
    pub name: String,
    pub value: AttrValue,
    pub line: usize,
    pub col: usize,
}

#[derive(Debug)]
pub enum Node {
    Element(Element),
    /// Consecutive `|` lines, one segment list per line (SPEC §6.2).
    TextBlock(Vec<Vec<TextPart>>),
    /// Raw passthrough lines, dedented by the marker's indent (SPEC §8).
    /// Verbatim by definition — no interpolation.
    Raw(Vec<String>),
    /// Comment lines (SPEC §3.1). `emit: true` for `//!` (HTML comment output);
    /// `emit: false` for `//` (kept in the AST so `fhtml fmt` preserves them,
    /// but never emitted as HTML).
    Comment {
        lines: Vec<String>,
        emit: bool,
    },
    Doctype,
    /// `if`/`elif`/`else` chain (SPEC §10.1).
    If(IfChain),
    /// `for … in …` with optional `empty` block (SPEC §10.2).
    For(ForLoop),
    /// `+name(args)` component call (SPEC §10.4).
    Call(Call),
    /// `children` — emits the caller's block; only legal inside a `def`
    /// body (SPEC §10.3), which the parser enforces.
    Children {
        line: usize,
    },
    /// Where a `def` sat in the source — an index into [`Document::defs`].
    /// Emits nothing; it exists so `fhtml fmt` reprints the definition in
    /// place instead of hoisting it past comments and markup. Top level
    /// only, like `def` itself.
    DefSite(usize),
}

#[derive(Debug)]
pub struct IfChain {
    /// The `if` arm followed by any `elif` arms, in source order.
    pub arms: Vec<IfArm>,
    pub else_body: Option<Vec<Node>>,
    pub line: usize,
}

#[derive(Debug)]
pub struct IfArm {
    pub cond: TplExpr,
    pub body: Vec<Node>,
}

#[derive(Debug)]
pub struct ForLoop {
    pub var: String,
    /// `for name, index in expr` — position for lists, key for maps.
    pub index: Option<String>,
    pub iter: TplExpr,
    pub body: Vec<Node>,
    /// Renders when the iterable is empty or `null`.
    pub empty: Option<Vec<Node>>,
    pub line: usize,
}

#[derive(Debug)]
pub enum AttrValue {
    Bool,
    /// Quoted (or literal unquoted) value as text segments. Raw `{!…}` is
    /// forbidden in attribute values (SPEC §9.1).
    Str(Vec<TextPart>),
    /// Unquoted `name={expr}` — the expression is the entire value.
    Expr(TplExpr),
}

#[derive(Debug)]
pub struct Element {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<ClassItem>,
    pub attrs: Vec<(String, AttrValue)>,
    pub text: Option<Vec<TextPart>>,
    /// Sole inline child via `>` (SPEC §4.6).
    pub chain: Option<Box<Element>>,
    /// Indented children; for chains these belong to the innermost element.
    pub children: Vec<Node>,
    pub line: usize,
}

impl Element {
    fn new(tag: &str, line: usize) -> Self {
        Element {
            tag: tag.to_string(),
            id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            text: None,
            chain: None,
            children: Vec::new(),
            line,
        }
    }
}

/// Returns the innermost element of a `>` chain (children attach here, SPEC §4.6).
fn innermost(el: &mut Element) -> &mut Element {
    if el.chain.is_some() {
        innermost(el.chain.as_mut().unwrap())
    } else {
        el
    }
}

/// Finds the first template construct in a document, for the static-only
/// `compile` path (SPEC §11). A `def` is a template construct: its body
/// binds parameters at render time.
pub fn first_template_use_doc(doc: &Document) -> Option<(usize, usize, String)> {
    let def = doc.defs.first().map(|d| (d.line, 1, "`def`".to_string()));
    let body = first_template_use(&doc.body);
    match (def, body) {
        (Some(d), Some(b)) => Some(if d.0 <= b.0 { d } else { b }),
        (d, b) => d.or(b),
    }
}

/// Finds the first template construct in the tree, for the static-only
/// `compile` path and for error reporting. Returns (line, col, description).
pub fn first_template_use(nodes: &[Node]) -> Option<(usize, usize, String)> {
    for node in nodes {
        match node {
            Node::If(c) => return Some((c.line, 1, "`if`".to_string())),
            Node::For(f) => return Some((f.line, 1, "`for`".to_string())),
            Node::Call(c) => return Some((c.line, 1, format!("`+{}`", c.name))),
            Node::Children { line } => return Some((*line, 1, "`children`".to_string())),
            Node::TextBlock(lines) => {
                if let Some(t) = lines.iter().find_map(|parts| interp_in(parts)) {
                    return Some((t.line, t.col, "`{…}` interpolation".to_string()));
                }
            }
            Node::Element(el) => {
                if let Some(found) = element_template_use(el) {
                    return Some(found);
                }
            }
            // Top-level only; `first_template_use_doc` reports defs from
            // `Document::defs`, which carries their lines.
            Node::DefSite(_) => {}
            Node::Raw(_) | Node::Comment { .. } | Node::Doctype => {}
        }
    }
    None
}

fn interp_in(parts: &[TextPart]) -> Option<&TplExpr> {
    parts.iter().find_map(|p| match p {
        TextPart::Interp { expr, .. } => Some(expr),
        TextPart::Lit(_) => None,
    })
}

fn element_template_use(el: &Element) -> Option<(usize, usize, String)> {
    for class in &el.classes {
        if let ClassItem::Interp(t) = class {
            return Some((t.line, t.col, "`{…}` interpolation".to_string()));
        }
    }
    for (_, value) in &el.attrs {
        match value {
            AttrValue::Expr(t) => return Some((t.line, t.col, "`{…}` interpolation".to_string())),
            AttrValue::Str(parts) => {
                if let Some(t) = interp_in(parts) {
                    return Some((t.line, t.col, "`{…}` interpolation".to_string()));
                }
            }
            AttrValue::Bool => {}
        }
    }
    if let Some(t) = el.text.as_deref().and_then(interp_in) {
        return Some((t.line, t.col, "`{…}` interpolation".to_string()));
    }
    if let Some(chain) = &el.chain {
        if let Some(found) = element_template_use(chain) {
            return Some(found);
        }
    }
    first_template_use(&el.children)
}

fn describe_indent(s: &str) -> String {
    if s.is_empty() {
        "none".to_string()
    } else {
        let kind = if s.starts_with('\t') { "tab" } else { "space" };
        format!("{} {kind}{}", s.len(), if s.len() == 1 { "" } else { "s" })
    }
}

struct Line {
    indent: String,
    content: String,
    num: usize,
}

/// Parses a source file into a [`Document`] plus non-fatal warnings (SPEC §2
/// rule 5). With `templates: false`, any template construct is an error
/// (SPEC §9.2).
pub fn parse(src: &str, templates: bool) -> Result<(Document, Vec<String>)> {
    let mut parser = Parser::new(src, templates);
    let body = parser.parse_block(0)?;
    if parser.pos < parser.lines.len() {
        let l = &parser.lines[parser.pos];
        return err(l.num, 1, "unexpected dedent");
    }
    Ok((
        Document {
            defs: parser.defs,
            body,
        },
        parser.warnings,
    ))
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
    /// Stack of open indents, one per level; `stack[0]` is `""` (SPEC §2).
    stack: Vec<String>,
    /// First observed indent step (in chars) — deviations warn, not error.
    step: Option<usize>,
    warnings: Vec<String>,
    templates: bool,
    /// `#!shorthand` seen: bare class tokens are decoded through the codebook
    /// (SPEC §3.x). Off until the directive appears.
    shorthand: bool,
    /// Component definitions, in source order (top-level only, SPEC §10.3).
    defs: Vec<Def>,
    /// Inside a `def` body — the only place `children` is legal. A plain
    /// bool suffices because `def`s cannot nest.
    in_def: bool,
}

impl Parser {
    fn new(src: &str, templates: bool) -> Self {
        let normalized = src.replace("\r\n", "\n");
        let lines = normalized
            .split('\n')
            .enumerate()
            .map(|(i, raw)| {
                let body = raw.trim_start_matches([' ', '\t']);
                let indent = raw[..raw.len() - body.len()].to_string();
                let content = body.trim_end().to_string();
                let indent = if content.is_empty() {
                    String::new()
                } else {
                    indent
                };
                Line {
                    indent,
                    content,
                    num: i + 1,
                }
            })
            .collect();
        Parser {
            lines,
            pos: 0,
            stack: vec![String::new()],
            step: None,
            warnings: Vec::new(),
            templates,
            shorthand: false,
            defs: Vec::new(),
            in_def: false,
        }
    }

    /// Level of the line at `idx` under the Python-style indent-stack rule
    /// (SPEC §2): exact match with an open level, or any extension of the
    /// innermost level opens one child level. Repeated calls on the same line
    /// are stable (pops and pushes are idempotent on re-classification).
    fn level_of(&mut self, idx: usize) -> Result<usize> {
        let num = self.lines[idx].num;
        let indent = self.lines[idx].indent.clone();
        if let Some(first) = indent.chars().next() {
            if indent.chars().any(|c| c != first) {
                return err(num, 1, "mixed tabs and spaces in indentation");
            }
        }
        if let Some(k) = self.stack.iter().position(|s| *s == indent) {
            self.stack.truncate(k + 1);
            return Ok(k);
        }
        let top = self.stack.last().unwrap().clone();
        if indent.len() > top.len() && indent.starts_with(&top) {
            let step = indent.len() - top.len();
            match self.step {
                None => self.step = Some(step),
                Some(usual) if usual != step => self.warnings.push(format!(
                    "{num}:1: warning: indent step of {step} differs from this file's usual {usual} — check the nesting here (`fhtml fmt` normalizes indentation)"
                )),
                _ => {}
            }
            self.stack.push(indent);
            return Ok(self.stack.len() - 1);
        }
        let open = self
            .stack
            .iter()
            .map(|s| describe_indent(s))
            .collect::<Vec<_>>()
            .join(", ");
        let hint = match (indent.chars().next(), top.chars().next()) {
            (Some(a), Some(b)) if a != b => {
                if a == '\t' {
                    " (this line indents with tabs but earlier lines use spaces)"
                } else {
                    " (this line indents with spaces but earlier lines use tabs)"
                }
            }
            _ => "",
        };
        err(
            num,
            1,
            format!(
                "indentation of {} matches no open level (open: {open}) — align exactly with an open level, or indent past the innermost to nest{hint}",
                describe_indent(&indent)
            ),
        )
    }

    fn parse_block(&mut self, depth: usize) -> Result<Vec<Node>> {
        let mut nodes = Vec::new();
        while self.pos < self.lines.len() {
            if self.lines[self.pos].content.is_empty() {
                self.pos += 1;
                continue;
            }
            let d = self.level_of(self.pos)?;
            let num = self.lines[self.pos].num;
            if d < depth {
                break;
            }
            if d > depth {
                return err(
                    num,
                    1,
                    "unexpected indentation — the line above cannot have children",
                );
            }
            let content = self.lines[self.pos].content.clone();
            let indent = self.lines[self.pos].indent.clone();

            if content.starts_with("//") {
                let emit = content.starts_with("//!");
                let rest = &content[if emit { 3 } else { 2 }..];
                let first = rest.strip_prefix(' ').unwrap_or(rest).to_string();
                self.pos += 1;
                let mut lines = vec![first];
                lines.extend(self.consume_deeper(&indent));
                nodes.push(Node::Comment { lines, emit });
            } else if content.starts_with('<') {
                self.pos += 1;
                let mut lines = vec![content];
                lines.extend(self.consume_deeper(&indent));
                nodes.push(Node::Raw(lines));
            } else if content.starts_with('|') {
                nodes.push(Node::TextBlock(self.parse_text_block(depth)?));
            } else {
                let first_tok = content.split_whitespace().next().unwrap();
                if content == "#!shorthand" {
                    // File-level directive: enable class shorthand for the rest
                    // of the file. Emits no node.
                    self.shorthand = true;
                    self.pos += 1;
                } else if first_tok == "doctype" {
                    let rest = content["doctype".len()..].trim();
                    if !(rest.is_empty() || rest == "html") {
                        return err(
                            num,
                            1,
                            "`doctype` takes nothing but an optional `html` (SPEC §7)",
                        );
                    }
                    self.pos += 1;
                    nodes.push(Node::Doctype);
                } else if RESERVED.contains(&first_tok)
                    || (first_tok.starts_with('+') && first_tok.len() > 1)
                {
                    let node = self.parse_statement(depth, first_tok, num)?;
                    nodes.push(node);
                } else {
                    let (logical, start) = self.join_continuations()?;
                    let mut cur = Cur::new(&logical, start);
                    let mut el = parse_element(&mut cur, self.templates, self.shorthand)?;
                    let children = self.parse_block(depth + 1)?;
                    innermost(&mut el).children = children;
                    check_void_content(&el)?;
                    nodes.push(Node::Element(el));
                }
            }
        }
        Ok(nodes)
    }

    /// Dispatches a line whose first token is a statement keyword or `+call`.
    /// A `def` yields a [`Node::DefSite`] marker — the definition itself goes
    /// to `self.defs`; the marker keeps its source position for `fhtml fmt`.
    fn parse_statement(&mut self, depth: usize, first_tok: &str, num: usize) -> Result<Node> {
        if !self.templates {
            return err(
                num,
                1,
                format!(
                    "`{first_tok}` is a template construct — not allowed with `--no-templates` (SPEC §9.2)"
                ),
            );
        }
        match first_tok {
            "if" => self.parse_if_chain(depth),
            "for" => self.parse_for(depth),
            "def" => {
                self.parse_def(depth)?;
                Ok(Node::DefSite(self.defs.len() - 1))
            }
            "children" => self.parse_children(),
            "elif" => err(
                num,
                1,
                "`elif` must directly follow an `if` or `elif` block at the same indent",
            ),
            "else" => err(
                num,
                1,
                "`else` must directly follow an `if` or `elif` block at the same indent",
            ),
            "empty" => err(
                num,
                1,
                "`empty` must directly follow a `for` block at the same indent",
            ),
            "include" => err(
                num,
                1,
                "`include` is part of the composition layer and is not implemented yet (SPEC §10.5)",
            ),
            _ => self.parse_call(depth),
        }
    }

    /// `def name(param param=default …)` + body (SPEC §10.3). Top level only;
    /// the definition goes to `self.defs`, not the node tree.
    fn parse_def(&mut self, depth: usize) -> Result<()> {
        let (content, line) = self.join_continuations()?;
        if depth != 0 {
            return err(
                line,
                1,
                "`def` is allowed only at top level — not nested in elements, statements, or other `def`s (SPEC §10.3)",
            );
        }
        let mut cur = Cur::at(&content, 3, line);
        cur.eat_ws();
        let col = cur.col();
        let Some(name) = read_name(&mut cur) else {
            return err(line, col, "`def` needs a component name: `def card(title)`");
        };
        if RESERVED.contains(&name.as_str())
            || matches!(name.as_str(), "ctx" | "true" | "false" | "null")
        {
            return err(
                line,
                col,
                format!("`{name}` is a reserved word and cannot name a component"),
            );
        }
        if let Some(prev) = self.defs.iter().find(|d| d.name == name) {
            return err(
                line,
                col,
                format!(
                    "component `{name}` is already defined on line {} — component names share one namespace per file (SPEC §10.3)",
                    prev.line
                ),
            );
        }
        let params = if cur.peek() == Some('(') {
            parse_params(&mut cur)?
        } else {
            Vec::new()
        };
        cur.eat_ws();
        if !cur.at_end() {
            let tok = cur.read_token();
            return err(
                line,
                col,
                format!("unexpected `{tok}` after the parameter list — a `def` line is `def name(params)` alone, with the body indented (SPEC §10.3)"),
            );
        }
        self.in_def = true;
        let body = self.statement_body(depth, line, "def");
        self.in_def = false;
        self.defs.push(Def {
            name,
            params,
            body: body?,
            line,
        });
        Ok(())
    }

    /// `children`, alone on its line, inside a `def` body only (SPEC §10.3).
    fn parse_children(&mut self) -> Result<Node> {
        let (content, line) = self.join_continuations()?;
        if !self.in_def {
            return err(
                line,
                1,
                "`children` is only allowed inside a `def` body — it emits the block the caller gave the component (SPEC §10.3)",
            );
        }
        if content.trim() != "children" {
            return err(
                line,
                1,
                "`children` takes nothing — it emits the caller's block (SPEC §10.3)",
            );
        }
        Ok(Node::Children { line })
    }

    /// `+name(args)` + optional indented block (the component's `children`,
    /// SPEC §10.4). Whether `name` resolves is checked at render time.
    fn parse_call(&mut self, depth: usize) -> Result<Node> {
        let (content, line) = self.join_continuations()?;
        let mut cur = Cur::new(&content, line);
        cur.bump(); // +
        let col = cur.col();
        let Some(name) = read_name(&mut cur) else {
            return err(
                line,
                col,
                "`+` starts a component call and needs a name: `+card(title=\"…\")`",
            );
        };
        let args = if cur.peek() == Some('(') {
            parse_args(&mut cur, self.templates)?
        } else {
            Vec::new()
        };
        cur.eat_ws();
        if !cur.at_end() {
            let junk_col = cur.col();
            let tok = cur.read_token();
            return err(
                line,
                junk_col,
                format!("unexpected `{tok}` after the component call — a call is `+{name}(args)` alone, with children on indented lines (SPEC §10.4)"),
            );
        }
        let children = self.parse_block(depth + 1)?;
        Ok(Node::Call(Call {
            name,
            args,
            children,
            line,
        }))
    }

    /// `if expr` + block, then any directly-following `elif`/`else` siblings
    /// at the same indent (SPEC §10.1 — no other siblings between).
    fn parse_if_chain(&mut self, depth: usize) -> Result<Node> {
        let (content, line) = self.join_continuations()?;
        let if_line = line;
        let cond = statement_expr(&content, 2, line, "if")?;
        let body = self.statement_body(depth, line, "if")?;
        let mut arms = vec![IfArm { cond, body }];
        let mut else_body = None;
        while let Some(idx) = self.peek_sibling(depth)? {
            let first = self.lines[idx].content.split_whitespace().next().unwrap();
            match first {
                "elif" => {
                    self.pos = idx;
                    let (content, line) = self.join_continuations()?;
                    let cond = statement_expr(&content, 4, line, "elif")?;
                    let body = self.statement_body(depth, line, "elif")?;
                    arms.push(IfArm { cond, body });
                }
                "else" => {
                    self.pos = idx;
                    let (content, line) = self.join_continuations()?;
                    if content.trim() != "else" {
                        return err(
                            line,
                            1,
                            "`else` takes no condition — use `elif <expr>` for another branch",
                        );
                    }
                    else_body = Some(self.statement_body(depth, line, "else")?);
                    break;
                }
                _ => break,
            }
        }
        Ok(Node::If(IfChain {
            arms,
            else_body,
            line: if_line,
        }))
    }

    /// `for name[, name] in expr` + block, then an optional directly-following
    /// `empty` sibling at the same indent (SPEC §10.2).
    fn parse_for(&mut self, depth: usize) -> Result<Node> {
        let (content, line) = self.join_continuations()?;
        let mut cur = Cur::at(&content, 3, line);
        cur.eat_ws();
        let var = loop_name(&mut cur, line, "a loop variable: `for item in items`")?;
        cur.eat_ws();
        let index = if cur.peek() == Some(',') {
            cur.bump();
            cur.eat_ws();
            let name = loop_name(&mut cur, line, "an index name after `,`")?;
            if name == var {
                return err(line, cur.col(), "loop variable and index must differ");
            }
            Some(name)
        } else {
            None
        };
        cur.eat_ws();
        let kw_col = cur.col();
        if read_name(&mut cur).as_deref() != Some("in") {
            return err(line, kw_col, "expected `in`: `for item[, index] in expr`");
        }
        let iter = statement_expr(&content, cur.i, line, "for")?;
        let body = self.statement_body(depth, line, "for")?;
        let mut empty = None;
        if let Some(idx) = self.peek_sibling(depth)? {
            if self.lines[idx].content.split_whitespace().next() == Some("empty") {
                self.pos = idx;
                let (content, eline) = self.join_continuations()?;
                if content.trim() != "empty" {
                    return err(
                        eline,
                        1,
                        "`empty` takes nothing — it renders when the `for` iterable is empty",
                    );
                }
                empty = Some(self.statement_body(depth, eline, "empty")?);
            }
        }
        Ok(Node::For(ForLoop {
            var,
            index,
            iter,
            body,
            empty,
            line,
        }))
    }

    /// A statement's indented block; empty is an error (a bare statement is
    /// always an indentation mistake).
    fn statement_body(&mut self, depth: usize, line: usize, kw: &str) -> Result<Vec<Node>> {
        let body = self.parse_block(depth + 1)?;
        if body.is_empty() {
            return err(line, 1, format!("`{kw}` needs an indented block"));
        }
        Ok(body)
    }

    /// Index of the next non-blank line if it sits at exactly `depth`.
    /// Does not consume — used to chain `elif`/`else`/`empty` siblings.
    fn peek_sibling(&mut self, depth: usize) -> Result<Option<usize>> {
        let mut j = self.pos;
        while j < self.lines.len() && self.lines[j].content.is_empty() {
            j += 1;
        }
        if j < self.lines.len() && self.level_of(j)? == depth {
            Ok(Some(j))
        } else {
            Ok(None)
        }
    }

    /// Consumes physical lines deeper-indented than `marker` (plus interior blanks),
    /// dedented by the marker's indent. Used for raw and comment blocks — these
    /// lines are verbatim and bypass indent-unit checking (SPEC §8).
    fn consume_deeper(&mut self, marker: &str) -> Vec<String> {
        let mut out = Vec::new();
        while self.pos < self.lines.len() {
            let l = &self.lines[self.pos];
            if l.content.is_empty() {
                out.push(String::new());
                self.pos += 1;
            } else if l.indent.len() > marker.len() && l.indent.starts_with(marker) {
                out.push(format!("{}{}", &l.indent[marker.len()..], l.content));
                self.pos += 1;
            } else {
                break;
            }
        }
        while out.last().is_some_and(|s| s.is_empty()) {
            out.pop();
        }
        out
    }

    /// Consecutive `|` lines at the same depth form one text block (SPEC §6.2).
    fn parse_text_block(&mut self, depth: usize) -> Result<Vec<Vec<TextPart>>> {
        let mut lines = Vec::new();
        loop {
            let content = self.lines[self.pos].content.clone();
            let num = self.lines[self.pos].num;
            let rest = &content[1..];
            let skip = if rest.starts_with(' ') { 2 } else { 1 };
            lines.push(parse_block_text_line(&content, skip, num, self.templates)?);
            self.pos += 1;

            let mut j = self.pos;
            while j < self.lines.len() && self.lines[j].content.is_empty() {
                j += 1;
            }
            if j < self.lines.len()
                && self.lines[j].content.starts_with('|')
                && self.level_of(j)? == depth
            {
                self.pos = j;
            } else {
                break;
            }
        }
        Ok(lines)
    }

    /// Joins `\`-continued physical lines into one logical line (SPEC §1).
    /// Called for element and statement lines — comments, raw, and text
    /// blocks never join.
    fn join_continuations(&mut self) -> Result<(String, usize)> {
        let start = self.lines[self.pos].num;
        let mut s = self.lines[self.pos].content.clone();
        self.pos += 1;
        while s.ends_with('\\') {
            s.pop();
            let s_trimmed = s.trim_end().to_string();
            if self.pos >= self.lines.len() || self.lines[self.pos].content.is_empty() {
                return err(start, 1, "line continuation `\\` with nothing to join");
            }
            s = format!("{} {}", s_trimmed, self.lines[self.pos].content);
            self.pos += 1;
        }
        Ok((s, start))
    }
}

/// Parses the expression that trails a statement keyword. `from` is the byte
/// offset in `content` where the expression region starts (whitespace ok).
fn statement_expr(content: &str, from: usize, line: usize, kw: &str) -> Result<TplExpr> {
    let rest = &content[from..];
    let expr_off = from + (rest.len() - rest.trim_start().len());
    let src = rest.trim();
    if src.is_empty() {
        return err(
            line,
            1,
            format!("`{kw}` needs an expression, e.g. `{}`", kw_example(kw)),
        );
    }
    match expr::parse(src) {
        Ok(e) => Ok(TplExpr {
            src: src.to_string(),
            expr: e,
            line,
            col: content[..expr_off].chars().count() + 1,
        }),
        Err(pe) => err(
            line,
            content[..expr_off + pe.offset].chars().count() + 1,
            pe.msg,
        ),
    }
}

fn kw_example(kw: &str) -> &'static str {
    match kw {
        "elif" => "elif user.invited",
        "for" => "for item in items",
        _ => "if user.active",
    }
}

/// A loop-binding name for `for` (SPEC §10.2). Expression literals and the
/// unshadowable `ctx` root (SPEC §9.4) are rejected.
fn loop_name(cur: &mut Cur, line: usize, what: &str) -> Result<String> {
    let col = cur.col();
    match read_name(cur) {
        Some(name) => match name.as_str() {
            "ctx" => err(
                line,
                col,
                "`ctx` is the reserved context root and cannot be shadowed (SPEC §9.4)",
            ),
            "true" | "false" | "null" => err(
                line,
                col,
                format!("`{name}` is an expression literal and cannot be a loop variable"),
            ),
            "in" => err(line, col, "`in` cannot be a loop variable"),
            _ => Ok(name),
        },
        None => err(line, col, format!("`for` needs {what}")),
    }
}

/// A parameter-list binding name (`def` params, SPEC §10.3): an identifier,
/// not `ctx` (unshadowable, SPEC §9.4), not an expression literal. `hint`
/// trails the unexpected-character error — the common mistake there is an
/// unbraced spaced expression value, so callers point at their brace syntax.
fn param_name(cur: &mut Cur, what: &str, hint: &str) -> Result<String> {
    let line = cur.line;
    let col = cur.col();
    match read_name(cur) {
        Some(name) => match name.as_str() {
            "ctx" => err(
                line,
                col,
                "`ctx` is the reserved context root and cannot be shadowed (SPEC §9.4)",
            ),
            "true" | "false" | "null" => err(
                line,
                col,
                format!("`{name}` is an expression literal and cannot be {what}"),
            ),
            _ => Ok(name),
        },
        None => match cur.peek() {
            Some(c) => err(
                line,
                col,
                format!(
                    "unexpected `{c}` — {what} is an identifier ([A-Za-z_][A-Za-z0-9_]*){hint}"
                ),
            ),
            None => unreachable!("callers check for `)` and end of line first"),
        },
    }
}

/// Parses `(param param=default …)` of a `def` (SPEC §10.3). The cursor sits
/// on `(`. Defaults are expressions, never strings; an unquoted default must
/// be whitespace-free, `{…}` braces a spaced one.
fn parse_params(cur: &mut Cur) -> Result<Vec<Param>> {
    let line = cur.line;
    cur.bump(); // (
    let mut params: Vec<Param> = Vec::new();
    loop {
        cur.eat_ws();
        match cur.peek() {
            None => return err(line, cur.col(), "unclosed parameter list — missing `)`"),
            Some(')') => {
                cur.bump();
                return Ok(params);
            }
            Some(_) => {
                let col = cur.col();
                let name = param_name(
                    cur,
                    "a parameter name",
                    "; a default expression with spaces needs braces: `limit={ctx.pageSize - 1}` (SPEC §10.3)",
                )?;
                if params.iter().any(|p| p.name == name) {
                    return err(line, col, format!("duplicate parameter `{name}`"));
                }
                match cur.peek() {
                    None | Some(' ') | Some('\t') | Some(')') | Some('=') => {}
                    Some(c) => {
                        return err(
                            line,
                            cur.col(),
                            format!("unexpected `{c}` in parameter name — parameters are identifiers ([A-Za-z_][A-Za-z0-9_]*)"),
                        )
                    }
                }
                let default = if cur.peek() == Some('=') {
                    cur.bump();
                    Some(expr_value(cur, "default", "a default")?)
                } else {
                    None
                };
                params.push(Param { name, default });
            }
        }
    }
}

/// Parses `(name name=value …)` of a `+call` (SPEC §10.4). The cursor sits
/// on `(`. Attribute *shape*, expression *values*: bare name = `true`,
/// quoted = string with interpolation, unquoted = expression.
fn parse_args(cur: &mut Cur, templates: bool) -> Result<Vec<Arg>> {
    let line = cur.line;
    cur.bump(); // (
    let mut args: Vec<Arg> = Vec::new();
    loop {
        cur.eat_ws();
        match cur.peek() {
            None => return err(line, cur.col(), "unclosed argument list — missing `)`"),
            Some(')') => {
                cur.bump();
                return Ok(args);
            }
            Some(_) => {
                let col = cur.col();
                let name = param_name(
                    cur,
                    "an argument name",
                    "; an expression value with spaces needs braces: `n={a + b}` (SPEC §10.4)",
                )?;
                if args.iter().any(|a| a.name == name) {
                    return err(line, col, format!("duplicate argument `{name}`"));
                }
                let value = match cur.peek() {
                    None | Some(' ') | Some('\t') | Some(')') => AttrValue::Bool,
                    Some('=') => {
                        cur.bump();
                        match cur.peek() {
                            Some(q @ ('"' | '\'')) => {
                                AttrValue::Str(parse_quoted(cur, q, false, templates)?)
                            }
                            _ => AttrValue::Expr(expr_value(cur, "value", "an argument")?),
                        }
                    }
                    Some(c) => {
                        return err(
                            line,
                            cur.col(),
                            format!("unexpected `{c}` in argument name — argument names are identifiers matching the component's parameters (SPEC §10.4)"),
                        )
                    }
                };
                args.push(Arg {
                    name,
                    value,
                    line,
                    col,
                });
            }
        }
    }
}

/// An expression value after `=` in a parameter default or call argument:
/// `{…}` braces a spaced expression, otherwise the value runs to the next
/// whitespace or `)` and must parse as an expression (SPEC §10.3–§10.4).
fn expr_value(cur: &mut Cur, missing: &str, what: &str) -> Result<TplExpr> {
    let line = cur.line;
    match cur.peek() {
        Some('{') => {
            let bcol = cur.col();
            cur.bump(); // {
            if cur.peek() == Some('!') {
                return err(
                    line,
                    bcol,
                    format!("raw interpolation `{{!…}}` makes no sense here — {what} value is passed, not output"),
                );
            }
            let t = scan_expr(cur, bcol)?;
            match cur.peek() {
                None | Some(' ') | Some('\t') | Some(')') => Ok(t),
                Some(_) => err(
                    line,
                    cur.col(),
                    format!("a braced `{{expr}}` must be the entire {missing} — quote to mix text: name=\"…\""),
                ),
            }
        }
        None | Some(' ') | Some('\t') | Some(')') => {
            err(line, cur.col(), format!("missing {missing} after `=`"))
        }
        Some(_) => {
            let start = cur.i;
            while let Some(c) = cur.peek() {
                if c == ' ' || c == '\t' || c == ')' {
                    break;
                }
                cur.bump();
            }
            let src = &cur.s[start..cur.i];
            match expr::parse(src) {
                Ok(e) => Ok(TplExpr {
                    src: src.to_string(),
                    expr: e,
                    line,
                    col: cur.col_at(start),
                }),
                Err(pe) => err(
                    line,
                    cur.col_at(start + pe.offset),
                    format!(
                        "{} — {what} value is an expression; quote a string (name=\"…\") or brace anything spaced (name={{a + b}})",
                        pe.msg
                    ),
                ),
            }
        }
    }
}

/// SPEC §7: void elements cannot have content. Children of chain elements were
/// checked in their own parse pass; this checks the chain path itself.
fn check_void_content(el: &Element) -> Result<()> {
    let mut cur = el;
    loop {
        if is_void(&cur.tag)
            && (cur.text.is_some() || cur.chain.is_some() || !cur.children.is_empty())
        {
            return err(
                cur.line,
                1,
                format!(
                    "`{}` is a void element and cannot have text or children",
                    cur.tag
                ),
            );
        }
        match &cur.chain {
            Some(next) => cur = next,
            None => break,
        }
    }
    Ok(())
}

/// Character cursor over one logical line.
struct Cur<'a> {
    s: &'a str,
    i: usize,
    line: usize,
}

impl<'a> Cur<'a> {
    fn new(s: &'a str, line: usize) -> Self {
        Cur { s, i: 0, line }
    }

    /// A cursor starting mid-line (statement bodies, text-block rests).
    fn at(s: &'a str, i: usize, line: usize) -> Self {
        Cur { s, i, line }
    }

    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn peek2(&self) -> Option<char> {
        let mut it = self.s[self.i..].chars();
        it.next();
        it.next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn col(&self) -> usize {
        self.s[..self.i].chars().count() + 1
    }

    fn col_at(&self, byte: usize) -> usize {
        self.s[..byte].chars().count() + 1
    }

    fn eat_ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.bump();
        }
    }

    fn at_end(&self) -> bool {
        self.i >= self.s.len()
    }

    /// Reads until whitespace. Lifetime is tied to the source, not the cursor.
    fn read_token(&mut self) -> &'a str {
        let src = self.s;
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == ' ' || c == '\t' {
                break;
            }
            self.bump();
        }
        &src[start..self.i]
    }
}

/// `[A-Za-z_][A-Za-z0-9_]*`, or `None` if the cursor doesn't start one.
fn read_name(cur: &mut Cur) -> Option<String> {
    match cur.peek() {
        Some(c) if c == '_' || c.is_ascii_alphabetic() => {}
        _ => return None,
    }
    let start = cur.i;
    while matches!(cur.peek(), Some(c) if c == '_' || c.is_ascii_alphanumeric()) {
        cur.bump();
    }
    Some(cur.s[start..cur.i].to_string())
}

const NO_TEMPLATES_INTERP: &str =
    "`{` starts an interpolation — not allowed with `--no-templates`; write `\\{` for a literal brace (SPEC §9.2)";

/// Scans a brace-delimited expression; the cursor sits just past `{` (and any
/// `!`). Consumes through the matching `}` — the first `}` outside the
/// expression's own string literals — and parses the inside (SPEC §9.3).
fn scan_expr(cur: &mut Cur, brace_col: usize) -> Result<TplExpr> {
    let line = cur.line;
    let start = cur.i;
    let mut quote: Option<char> = None;
    loop {
        match cur.peek() {
            None => {
                return err(
                    line,
                    brace_col,
                    "unclosed `{` — expected `}` to end the interpolation",
                )
            }
            Some('}') if quote.is_none() => break,
            Some(q @ ('\'' | '"')) => {
                match quote {
                    Some(open) if open == q => quote = None,
                    None => quote = Some(q),
                    _ => {}
                }
                cur.bump();
            }
            Some('\\') if quote.is_some() => {
                cur.bump();
                cur.bump();
            }
            Some(_) => {
                cur.bump();
            }
        }
    }
    let inner = &cur.s[start..cur.i];
    cur.bump(); // }
    match expr::parse(inner) {
        Ok(e) => Ok(TplExpr {
            src: inner.trim().to_string(),
            expr: e,
            line,
            col: brace_col,
        }),
        Err(pe) => err(line, cur.col_at(start + pe.offset), pe.msg),
    }
}

/// Parses one element from the cursor (SPEC §4). Recurses for `>` chains.
fn parse_element(cur: &mut Cur, templates: bool, shorthand: bool) -> Result<Element> {
    let src = cur.s;
    let line = cur.line;
    cur.eat_ws();
    let col = cur.col();

    let tag = match cur.peek() {
        None => return err(line, col, "expected an element"),
        Some('.') => {
            cur.bump();
            match cur.peek() {
                None | Some(' ') | Some('\t') | Some('(') => "div".to_string(),
                Some(_) => {
                    let word = cur.read_token();
                    return err(
                        line,
                        col,
                        format!(
                            "`.{word}` is Pug syntax, not fhtml — write `. {word}` (`.` alone is div; classes are bare tokens)"
                        ),
                    );
                }
            }
        }
        Some('#') => {
            let word = cur.read_token();
            return err(
                line,
                col,
                format!("`{word}` cannot start an element — write `. {word}`"),
            );
        }
        Some(c) if c.is_ascii_alphabetic() => {
            let start = cur.i;
            cur.bump();
            while let Some(c) = cur.peek() {
                if c.is_ascii_alphanumeric() || c == '-' {
                    cur.bump();
                } else {
                    break;
                }
            }
            let name = &src[start..cur.i];
            match cur.peek() {
                None | Some(' ') | Some('\t') | Some('(') => {}
                Some(c @ ('.' | '#')) => {
                    return err(
                        line,
                        cur.col(),
                        format!(
                            "`{name}{c}…` is Pug syntax, not fhtml — write classes as bare tokens (`{name} card`) and ids as `#id` tokens (`{name} #hero`)"
                        ),
                    );
                }
                Some(c) => {
                    return err(
                        line,
                        cur.col(),
                        format!("unexpected `{c}` after tag `{name}`"),
                    )
                }
            }
            if RESERVED.contains(&name) {
                return err(
                    line,
                    col,
                    format!(
                        "`{name}` is a reserved word — for an element literally named `{name}`, use raw HTML passthrough (SPEC §8)"
                    ),
                );
            }
            name.to_string()
        }
        Some('+') => {
            return err(
                line,
                col,
                "`+` starts a component call and needs a name butted to it: `+card(title=\"…\")` (SPEC §10.4)",
            )
        }
        Some(c) => return err(line, col, format!("unexpected `{c}` at start of element")),
    };

    let mut el = Element::new(&tag, line);

    if cur.peek() == Some('(') {
        parse_attrs(cur, &mut el, templates)?;
        match cur.peek() {
            None | Some(' ') | Some('\t') => {}
            Some(c) => {
                return err(
                    line,
                    cur.col(),
                    format!("expected space after `)`, found `{c}`"),
                )
            }
        }
    }

    loop {
        cur.eat_ws();
        let col = cur.col();
        let Some(c) = cur.peek() else { break };

        if c == '"' {
            if el.text.is_some() {
                return err(
                    line,
                    col,
                    "an element may have at most one inline text segment",
                );
            }
            el.text = Some(parse_quoted(cur, '"', true, templates)?);
            continue;
        }

        if c == '{' {
            // Class-position interpolation (SPEC §9.2): whole token, escaped
            // output, result splits on whitespace at render time.
            if el.text.is_some() {
                return err(
                    line,
                    col,
                    "only a `>` chain may follow inline text (order is `tag(attrs) classes \"text\"`)",
                );
            }
            el.classes
                .push(ClassItem::Interp(parse_class_interp(cur, templates)?));
            continue;
        }

        let tok = cur.read_token();
        if tok == ">" {
            cur.eat_ws();
            if cur.at_end() {
                return err(line, cur.col(), "expected an element after `>`");
            }
            if cur.peek() == Some('+') {
                return err(
                    line,
                    cur.col(),
                    "a component call cannot be the target of a `>` chain — write the call as an indented child (SPEC §10.4)",
                );
            }
            el.chain = Some(Box::new(parse_element(cur, templates, shorthand)?));
            break;
        }
        if el.text.is_some() {
            return err(
                line,
                col,
                "only a `>` chain may follow inline text (order is `tag(attrs) classes \"text\"`)",
            );
        }
        if let Some(id) = tok.strip_prefix('#') {
            if id.is_empty() {
                return err(line, col, "empty id");
            }
            if el.id.is_some() {
                return err(line, col, "an element may have at most one id");
            }
            el.id = Some(id.to_string());
            continue;
        }
        el.classes.push(ClassItem::Lit(class_token(tok, shorthand)));
    }

    Ok(el)
}

/// Resolves a bare class token to its literal class name. In `#!shorthand`
/// mode a recognized code is decoded (`ti4` → `text-indigo-400`); a leading
/// `=` escapes a token to stay verbatim (`=ti4` → `ti4`); anything else is
/// left untouched.
fn class_token(tok: &str, shorthand: bool) -> String {
    if !shorthand {
        return tok.to_string();
    }
    match tok.strip_prefix('=') {
        Some(literal) => literal.to_string(),
        None => crate::shorthand::decode(tok).unwrap_or_else(|| tok.to_string()),
    }
}

/// A `{expr}` token in class position. The cursor sits on `{`.
fn parse_class_interp(cur: &mut Cur, templates: bool) -> Result<TplExpr> {
    let line = cur.line;
    let col = cur.col();
    if !templates {
        return err(line, col, NO_TEMPLATES_INTERP);
    }
    cur.bump(); // {
    if cur.peek() == Some('!') {
        return err(
            line,
            col,
            "raw interpolation `{!…}` is not allowed in class position (SPEC §9.1) — for raw HTML content, use a text-block line: `| {!expr}`",
        );
    }
    let t = scan_expr(cur, col)?;
    match cur.peek() {
        None | Some(' ') | Some('\t') => Ok(t),
        Some(_) => err(
            line,
            cur.col(),
            "a class interpolation must be a whole token — never glue interpolation to class text \
             (Tailwind's scanner cannot see built names); interpolate whole class names instead",
        ),
    }
}

/// Parses `(name name=value …)` (SPEC §4.3). The cursor sits on `(`.
fn parse_attrs(cur: &mut Cur, el: &mut Element, templates: bool) -> Result<()> {
    let src = cur.s;
    let line = cur.line;
    cur.bump(); // (
    loop {
        cur.eat_ws();
        match cur.peek() {
            None => return err(line, cur.col(), "unclosed attribute list — missing `)`"),
            Some(')') => {
                cur.bump();
                return Ok(());
            }
            Some(_) => {
                let col = cur.col();
                let start = cur.i;
                while let Some(c) = cur.peek() {
                    if c == '=' || c == ' ' || c == '\t' || c == ')' {
                        break;
                    }
                    cur.bump();
                }
                let name = &src[start..cur.i];
                if name.is_empty() {
                    return err(line, col, "expected attribute name");
                }
                let value = if cur.peek() == Some('=') {
                    cur.bump();
                    match cur.peek() {
                        Some(q @ ('"' | '\'')) => {
                            AttrValue::Str(parse_quoted(cur, q, false, templates)?)
                        }
                        Some('{') => {
                            let bcol = cur.col();
                            if !templates {
                                return err(line, bcol, NO_TEMPLATES_INTERP);
                            }
                            cur.bump(); // {
                            if cur.peek() == Some('!') {
                                return err(
                                    line,
                                    bcol,
                                    "raw interpolation `{!…}` is forbidden inside attribute values (SPEC §9.1)",
                                );
                            }
                            let t = scan_expr(cur, bcol)?;
                            match cur.peek() {
                                None | Some(' ') | Some('\t') | Some(')') => AttrValue::Expr(t),
                                Some(_) => {
                                    return err(
                                        line,
                                        cur.col(),
                                        format!(
                                            "an unquoted `{{expr}}` must be the entire attribute value — quote to mix text: {name}=\"…\""
                                        ),
                                    )
                                }
                            }
                        }
                        None | Some(' ') | Some('\t') | Some(')') => {
                            return err(line, cur.col(), format!("missing value after `{name}=`"))
                        }
                        Some(_) => {
                            let vstart = cur.i;
                            while let Some(c) = cur.peek() {
                                if c == ' ' || c == '\t' || c == ')' {
                                    break;
                                }
                                cur.bump();
                            }
                            AttrValue::Str(lit_parts(&src[vstart..cur.i]))
                        }
                    }
                } else {
                    AttrValue::Bool
                };

                if name == "class" {
                    match value {
                        AttrValue::Str(parts) => merge_class_attr(el, parts)?,
                        AttrValue::Expr(t) => el.classes.push(ClassItem::Interp(t)),
                        AttrValue::Bool => {
                            return err(line, col, "`class` attribute requires a value")
                        }
                    }
                } else {
                    if el.attrs.iter().any(|(n, _)| n == name) {
                        return err(line, col, format!("duplicate attribute `{name}`"));
                    }
                    el.attrs.push((name.to_string(), value));
                }
            }
        }
    }
}

/// Merges a quoted `class="…"` value into the class list: literal runs split
/// on whitespace; an interpolation becomes one class item and must stand
/// whitespace-separated (glued fragments are invisible to Tailwind's scanner
/// and are rejected, the footgun rule).
fn merge_class_attr(el: &mut Element, parts: Vec<TextPart>) -> Result<()> {
    let mut boundary = true; // at value start / after whitespace
    for part in parts {
        match part {
            TextPart::Lit(s) => {
                if !boundary && !s.starts_with([' ', '\t']) {
                    let t = match el.classes.last() {
                        Some(ClassItem::Interp(t)) => t,
                        _ => unreachable!("boundary is false only after an interp"),
                    };
                    return err(t.line, t.col, GLUED_CLASS_ATTR);
                }
                el.classes
                    .extend(s.split_whitespace().map(|c| ClassItem::Lit(c.to_string())));
                boundary = s.ends_with([' ', '\t']);
            }
            TextPart::Interp { expr, .. } => {
                if !boundary {
                    return err(expr.line, expr.col, GLUED_CLASS_ATTR);
                }
                el.classes.push(ClassItem::Interp(expr));
                boundary = false;
            }
        }
    }
    Ok(())
}

const GLUED_CLASS_ATTR: &str =
    "interpolation in a class list must be a whole, whitespace-separated token — never glue \
     interpolation to class text (Tailwind's scanner cannot see built names)";

/// Parses a quoted string with SPEC escapes into text segments. Inline text
/// (§6.1) allows `\" \\ \{ \n` and raw `{!…}`; attribute values (§4.3) allow
/// `\" \' \\ \{`, raw forbidden (SPEC §9.1).
fn parse_quoted(
    cur: &mut Cur,
    quote: char,
    is_text: bool,
    templates: bool,
) -> Result<Vec<TextPart>> {
    let line = cur.line;
    cur.bump(); // opening quote
    let mut parts = Vec::new();
    let mut lit = String::new();
    loop {
        let col = cur.col();
        match cur.peek() {
            None => return err(line, cur.col(), "unclosed string"),
            Some(c) if c == quote => {
                cur.bump();
                flush(&mut parts, &mut lit);
                return Ok(parts);
            }
            Some('\\') => {
                cur.bump();
                match cur.bump() {
                    Some('"') => lit.push('"'),
                    Some('\'') if !is_text => lit.push('\''),
                    Some('\\') => lit.push('\\'),
                    Some('{') => lit.push('{'),
                    Some('n') if is_text => lit.push('\n'),
                    Some(c) => return err(line, cur.col(), format!("unknown escape `\\{c}`")),
                    None => return err(line, cur.col(), "unclosed string"),
                }
            }
            Some('{') => {
                if !templates {
                    return err(line, col, NO_TEMPLATES_INTERP);
                }
                cur.bump(); // {
                let raw = cur.peek() == Some('!');
                if raw {
                    if !is_text {
                        return err(
                            line,
                            col,
                            "raw interpolation `{!…}` is forbidden inside attribute values (SPEC §9.1)",
                        );
                    }
                    cur.bump(); // !
                }
                let expr = scan_expr(cur, col)?;
                flush(&mut parts, &mut lit);
                parts.push(TextPart::Interp { expr, raw });
            }
            Some(_) => lit.push(cur.bump().unwrap()),
        }
    }
}

/// One `|` text-block line into segments. Verbatim except `\{` (literal
/// brace) and `{…}`/`{!…}` interpolation (SPEC §6.2, §9.1).
fn parse_block_text_line(
    content: &str,
    from: usize,
    line: usize,
    templates: bool,
) -> Result<Vec<TextPart>> {
    let mut cur = Cur::at(content, from, line);
    let mut parts = Vec::new();
    let mut lit = String::new();
    loop {
        let col = cur.col();
        match cur.peek() {
            None => {
                flush(&mut parts, &mut lit);
                return Ok(parts);
            }
            Some('\\') if cur.peek2() == Some('{') => {
                cur.bump();
                cur.bump();
                lit.push('{');
            }
            Some('{') => {
                if !templates {
                    return err(line, col, NO_TEMPLATES_INTERP);
                }
                cur.bump(); // {
                let raw = cur.peek() == Some('!');
                if raw {
                    cur.bump(); // !
                }
                let expr = scan_expr(&mut cur, col)?;
                flush(&mut parts, &mut lit);
                parts.push(TextPart::Interp { expr, raw });
            }
            Some(_) => lit.push(cur.bump().unwrap()),
        }
    }
}

fn flush(parts: &mut Vec<TextPart>, lit: &mut String) {
    if !lit.is_empty() {
        parts.push(TextPart::Lit(std::mem::take(lit)));
    }
}
