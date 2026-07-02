//! Parser for the fhtml static markup layer (SPEC §1–§8).
//!
//! Template-layer constructs (SPEC §9–§10) are recognized and rejected with a
//! clear "not implemented in v0.1" error so the syntax space stays reserved.

use crate::error::{err, Result};

pub const RESERVED: [&str; 8] = [
    "if", "elif", "else", "for", "empty", "def", "slot", "include",
];

pub const VOID: [&str; 13] = [
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track",
    "wbr",
];

pub fn is_void(tag: &str) -> bool {
    VOID.contains(&tag)
}

#[derive(Debug)]
pub enum Node {
    Element(Element),
    /// Consecutive `|` lines, one string per line (SPEC §6.2).
    TextBlock(Vec<String>),
    /// Raw passthrough lines, dedented by the marker's indent (SPEC §8).
    Raw(Vec<String>),
    /// Comment lines (SPEC §3.1). `emit: true` for `//!` (HTML comment output);
    /// `emit: false` for `//` (kept in the AST so `fhtml fmt` preserves them,
    /// but never emitted as HTML).
    Comment {
        lines: Vec<String>,
        emit: bool,
    },
    Doctype,
}

#[derive(Debug)]
pub enum AttrValue {
    Bool,
    Str(String),
}

#[derive(Debug)]
pub struct Element {
    pub tag: String,
    pub id: Option<String>,
    pub classes: Vec<String>,
    pub attrs: Vec<(String, AttrValue)>,
    pub text: Option<String>,
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

/// Parses a source file into nodes plus non-fatal warnings (SPEC §2 rule 5).
pub fn parse(src: &str) -> Result<(Vec<Node>, Vec<String>)> {
    let mut parser = Parser::new(src);
    let nodes = parser.parse_block(0)?;
    if parser.pos < parser.lines.len() {
        let l = &parser.lines[parser.pos];
        return err(l.num, 1, "unexpected dedent");
    }
    Ok((nodes, parser.warnings))
}

struct Parser {
    lines: Vec<Line>,
    pos: usize,
    /// Stack of open indents, one per level; `stack[0]` is `""` (SPEC §2).
    stack: Vec<String>,
    /// First observed indent step (in chars) — deviations warn, not error.
    step: Option<usize>,
    warnings: Vec<String>,
}

impl Parser {
    fn new(src: &str) -> Self {
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
                if first_tok == "doctype" {
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
                    return err(
                        num,
                        1,
                        format!(
                            "`{first_tok}` is part of the template layer and is not implemented in v0.1"
                        ),
                    );
                } else {
                    let (logical, start) = self.join_continuations()?;
                    let mut cur = Cur::new(&logical, start);
                    let mut el = parse_element(&mut cur)?;
                    let children = self.parse_block(depth + 1)?;
                    innermost(&mut el).children = children;
                    check_void_content(&el)?;
                    nodes.push(Node::Element(el));
                }
            }
        }
        Ok(nodes)
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
    fn parse_text_block(&mut self, depth: usize) -> Result<Vec<String>> {
        let mut lines = Vec::new();
        loop {
            let content = &self.lines[self.pos].content;
            let rest = &content[1..];
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            lines.push(rest.replace("\\{", "{"));
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
    /// Only called for element lines — comments, raw, and text blocks never join.
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

    fn peek(&self) -> Option<char> {
        self.s[self.i..].chars().next()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.i += c.len_utf8();
        Some(c)
    }

    fn col(&self) -> usize {
        self.s[..self.i].chars().count() + 1
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

/// Parses one element from the cursor (SPEC §4). Recurses for `>` chains.
fn parse_element(cur: &mut Cur) -> Result<Element> {
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
        Some(c) => return err(line, col, format!("unexpected `{c}` at start of element")),
    };

    let mut el = Element::new(&tag, line);

    if cur.peek() == Some('(') {
        parse_attrs(cur, &mut el)?;
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
            el.text = Some(parse_quoted(cur, '"', true)?);
            continue;
        }

        let tok = cur.read_token();
        if tok == ">" {
            cur.eat_ws();
            if cur.at_end() {
                return err(line, cur.col(), "expected an element after `>`");
            }
            el.chain = Some(Box::new(parse_element(cur)?));
            break;
        }
        if el.text.is_some() {
            return err(
                line,
                col,
                "only a `>` chain may follow inline text (order is `tag(attrs) classes \"text\"`)",
            );
        }
        if tok.starts_with('{') {
            return err(
                line,
                col,
                "`{…}` interpolation is part of the template layer and is not implemented in v0.1",
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
        el.classes.push(tok.to_string());
    }

    Ok(el)
}

/// Parses `(name name=value …)` (SPEC §4.3). The cursor sits on `(`.
fn parse_attrs(cur: &mut Cur, el: &mut Element) -> Result<()> {
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
                        Some(q @ ('"' | '\'')) => AttrValue::Str(parse_quoted(cur, q, false)?),
                        Some('{') => {
                            return err(
                                line,
                                cur.col(),
                                "`{…}` expressions are part of the template layer and are not implemented in v0.1",
                            )
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
                            AttrValue::Str(src[vstart..cur.i].to_string())
                        }
                    }
                } else {
                    AttrValue::Bool
                };

                if name == "class" {
                    match value {
                        AttrValue::Str(v) => {
                            el.classes.extend(v.split_whitespace().map(String::from))
                        }
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

/// Parses a quoted string with SPEC escapes. Inline text (§6.1) allows
/// `\" \\ \{ \n`; attribute values (§4.3) allow `\" \' \\ \{`.
fn parse_quoted(cur: &mut Cur, quote: char, is_text: bool) -> Result<String> {
    let line = cur.line;
    cur.bump(); // opening quote
    let mut out = String::new();
    loop {
        match cur.bump() {
            None => return err(line, cur.col(), "unclosed string"),
            Some(c) if c == quote => return Ok(out),
            Some('\\') => match cur.bump() {
                Some('"') => out.push('"'),
                Some('\'') if !is_text => out.push('\''),
                Some('\\') => out.push('\\'),
                Some('{') => out.push('{'),
                Some('n') if is_text => out.push('\n'),
                Some(c) => return err(line, cur.col(), format!("unknown escape `\\{c}`")),
                None => return err(line, cur.col(), "unclosed string"),
            },
            Some(c) => out.push(c),
        }
    }
}
