//! The template-layer expression mini-language (SPEC §9.3–§9.4).
//!
//! The grammar is closed — no function calls, no lambdas, no host-language
//! escape — and is identical across all render backends. [`parse`] turns an
//! expression's source text into an AST; parse errors carry a byte offset
//! within the expression so the fhtml parser can map them to a file
//! line/column. [`eval`] evaluates an AST against a [`Scope`] plus the
//! host-provided `ctx` value, which resolves in every scope and cannot be
//! shadowed (SPEC §9.4).

use std::fmt;

// ---------------------------------------------------------------- values

/// A template-layer value (SPEC §9.4). Maps preserve insertion order.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    List(Vec<Value>),
    Map(Vec<(String, Value)>),
}

impl Value {
    /// SPEC §9.4 falsiness: `null`, `false`, `0`, `""`, empty list, empty map.
    pub fn truthy(&self) -> bool {
        match self {
            Value::Null => false,
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::Str(s) => !s.is_empty(),
            Value::List(v) => !v.is_empty(),
            Value::Map(m) => !m.is_empty(),
        }
    }

    /// Type name with article, for error messages ("a list", "null").
    fn describe(&self) -> &'static str {
        match self {
            Value::Null => "null",
            Value::Bool(_) => "a boolean",
            Value::Number(_) => "a number",
            Value::Str(_) => "a string",
            Value::List(_) => "a list",
            Value::Map(_) => "a map",
        }
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}
impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}
impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Value::Number(n as f64)
    }
}
impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::Str(s.to_string())
    }
}
impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::Str(s)
    }
}
impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::List(v.into_iter().map(Into::into).collect())
    }
}

/// SPEC §9.4 deep structural equality. Maps compare by key set and values,
/// independent of insertion order; values of different types are never equal.
pub fn deep_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Str(x), Value::Str(y)) => x == y,
        (Value::List(x), Value::List(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(a, b)| deep_eq(a, b))
        }
        (Value::Map(x), Value::Map(y)) => {
            x.len() == y.len()
                && x.iter().all(|(k, v)| {
                    y.iter()
                        .find(|(k2, _)| k2 == k)
                        .is_some_and(|(_, v2)| deep_eq(v, v2))
                })
        }
        _ => false,
    }
}

/// SPEC §9.4 stringification: `null` → `""`, booleans → `true`/`false`,
/// numbers in shortest round-trip decimal form — never exponent notation,
/// integral values without `.0`, `-0` as `0`. Lists and maps are an error
/// (catches mistakes early).
pub fn stringify(v: &Value) -> Result<String, RenderError> {
    match v {
        Value::Null => Ok(String::new()),
        Value::Bool(b) => Ok(b.to_string()),
        Value::Number(n) => Ok(fmt_number(*n)),
        Value::Str(s) => Ok(s.clone()),
        Value::List(_) | Value::Map(_) => Err(RenderError::new(format!(
            "cannot interpolate {} — interpolation takes scalars (string, number, boolean, null)",
            v.describe()
        ))),
    }
}

/// Number → string per the SPEC §9.4 rule (see [`stringify`]). Rust's `{}`
/// on `f64` already prints shortest-round-trip decimal without exponent
/// notation or a trailing `.0`; the only fixup is `-0` → `0`.
pub fn fmt_number(n: f64) -> String {
    if n == 0.0 {
        "0".to_string()
    } else {
        n.to_string()
    }
}

// ---------------------------------------------------------------- errors

/// A parse error inside an expression. `offset` is the byte offset within
/// the expression source text (not the file — the caller maps it).
#[derive(Debug)]
pub struct ExprError {
    pub offset: usize,
    pub msg: String,
}

impl fmt::Display for ExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for ExprError {}

/// An evaluation error (SPEC §9.4 — e.g. arithmetic on non-numbers). The
/// message names the offending operation; the renderer attaches the file
/// position of the enclosing interpolation or statement.
#[derive(Debug)]
pub struct RenderError {
    pub msg: String,
}

impl RenderError {
    pub fn new(msg: impl Into<String>) -> Self {
        RenderError { msg: msg.into() }
    }
}

impl fmt::Display for RenderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl std::error::Error for RenderError {}

// ------------------------------------------------------------------- AST

/// Expression AST for the closed grammar of SPEC §9.3.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Null,
    Bool(bool),
    Number(f64),
    Str(String),
    Name(String),
    /// `base.name`
    Field(Box<Expr>, String),
    /// `base[index]`
    Index(Box<Expr>, Box<Expr>),
    /// `!x`
    Not(Box<Expr>),
    /// `-x`
    Neg(Box<Expr>),
    /// `&&` — short-circuits; yields the deciding operand's value.
    And(Box<Expr>, Box<Expr>),
    /// `||` — short-circuits; yields the deciding operand's value.
    Or(Box<Expr>, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    /// `cond ? then : else` — only the taken branch is evaluated.
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
}

impl BinOp {
    fn sym(self) -> &'static str {
        match self {
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Mod => "%",
        }
    }
}

// ---------------------------------------------------------------- parsing

/// Parses one complete expression; trailing input is an error.
pub fn parse(src: &str) -> Result<Expr, ExprError> {
    let mut p = P { src, i: 0 };
    let e = p.expr()?;
    p.ws();
    if p.i < p.src.len() {
        return p.unexpected();
    }
    Ok(e)
}

struct P<'a> {
    src: &'a str,
    i: usize,
}

impl P<'_> {
    fn b(&self) -> Option<u8> {
        self.src.as_bytes().get(self.i).copied()
    }

    fn ws(&mut self) {
        while matches!(self.b(), Some(b' ' | b'\t')) {
            self.i += 1;
        }
    }

    /// Consumes `tok` if the input starts with it here.
    fn eat(&mut self, tok: &str) -> bool {
        if self.src[self.i..].starts_with(tok) {
            self.i += tok.len();
            true
        } else {
            false
        }
    }

    fn err<T>(&self, offset: usize, msg: impl Into<String>) -> Result<T, ExprError> {
        Err(ExprError {
            offset,
            msg: msg.into(),
        })
    }

    fn unexpected<T>(&self) -> Result<T, ExprError> {
        match self.src[self.i..].chars().next() {
            Some(c) => self.err(self.i, format!("unexpected `{c}` in expression")),
            None => self.err(self.i, "expected an expression"),
        }
    }

    /// Full expression = ternary (SPEC §9.3; right-associative via recursion).
    fn expr(&mut self) -> Result<Expr, ExprError> {
        let cond = self.or()?;
        self.ws();
        if !self.eat("?") {
            return Ok(cond);
        }
        let then = self.expr()?;
        self.ws();
        if !self.eat(":") {
            return self.err(self.i, "expected `:` for the ternary's else branch");
        }
        let els = self.expr()?;
        Ok(Expr::Ternary(Box::new(cond), Box::new(then), Box::new(els)))
    }

    fn or(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.and()?;
        loop {
            self.ws();
            if self.eat("||") {
                e = Expr::Or(Box::new(e), Box::new(self.and()?));
            } else {
                return Ok(e);
            }
        }
    }

    fn and(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.equality()?;
        loop {
            self.ws();
            if self.eat("&&") {
                e = Expr::And(Box::new(e), Box::new(self.equality()?));
            } else {
                return Ok(e);
            }
        }
    }

    fn equality(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.compare()?;
        loop {
            self.ws();
            let op = if self.eat("==") {
                BinOp::Eq
            } else if self.eat("!=") {
                BinOp::Ne
            } else {
                return Ok(e);
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.compare()?));
        }
    }

    fn compare(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.additive()?;
        loop {
            self.ws();
            let op = if self.eat("<=") {
                BinOp::Le
            } else if self.eat(">=") {
                BinOp::Ge
            } else if self.eat("<") {
                BinOp::Lt
            } else if self.eat(">") {
                BinOp::Gt
            } else {
                return Ok(e);
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.additive()?));
        }
    }

    fn additive(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.mult()?;
        loop {
            self.ws();
            let op = if self.eat("+") {
                BinOp::Add
            } else if self.eat("-") {
                BinOp::Sub
            } else {
                return Ok(e);
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.mult()?));
        }
    }

    fn mult(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.unary()?;
        loop {
            self.ws();
            let op = if self.eat("*") {
                BinOp::Mul
            } else if self.eat("/") {
                BinOp::Div
            } else if self.eat("%") {
                BinOp::Mod
            } else {
                return Ok(e);
            };
            e = Expr::Binary(op, Box::new(e), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr, ExprError> {
        self.ws();
        if self.eat("!") {
            Ok(Expr::Not(Box::new(self.unary()?)))
        } else if self.eat("-") {
            Ok(Expr::Neg(Box::new(self.unary()?)))
        } else {
            self.postfix()
        }
    }

    fn postfix(&mut self) -> Result<Expr, ExprError> {
        let mut e = self.primary()?;
        loop {
            self.ws();
            if self.eat(".") {
                self.ws();
                let at = self.i;
                match self.name() {
                    Some(n) => e = Expr::Field(Box::new(e), n),
                    None => return self.err(at, "expected a name after `.`"),
                }
            } else if self.eat("[") {
                let idx = self.expr()?;
                self.ws();
                if !self.eat("]") {
                    return self.err(self.i, "expected `]` to close the index");
                }
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else {
                return Ok(e);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, ExprError> {
        self.ws();
        match self.b() {
            Some(b'(') => {
                self.i += 1;
                let e = self.expr()?;
                self.ws();
                if !self.eat(")") {
                    return self.err(self.i, "expected `)`");
                }
                Ok(e)
            }
            Some(q @ (b'\'' | b'"')) => self.string(q),
            Some(c) if c.is_ascii_digit() => self.number(),
            Some(c) if c == b'_' || c.is_ascii_alphabetic() => {
                let name = self.name().unwrap();
                Ok(match name.as_str() {
                    "true" => Expr::Bool(true),
                    "false" => Expr::Bool(false),
                    "null" => Expr::Null,
                    _ => Expr::Name(name),
                })
            }
            _ => self.unexpected(),
        }
    }

    /// `[A-Za-z_][A-Za-z0-9_]*`, or `None` if the input doesn't start one.
    fn name(&mut self) -> Option<String> {
        match self.b() {
            Some(c) if c == b'_' || c.is_ascii_alphabetic() => {}
            _ => return None,
        }
        let start = self.i;
        while matches!(self.b(), Some(c) if c == b'_' || c.is_ascii_alphanumeric()) {
            self.i += 1;
        }
        Some(self.src[start..self.i].to_string())
    }

    /// Decimal integer or float: digits, optional fraction, optional exponent.
    fn number(&mut self) -> Result<Expr, ExprError> {
        let start = self.i;
        while matches!(self.b(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.b() == Some(b'.')
            && matches!(self.src.as_bytes().get(self.i + 1), Some(c) if c.is_ascii_digit())
        {
            self.i += 1;
            while matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.b(), Some(b'e' | b'E')) {
            let mut j = self.i + 1;
            if matches!(self.src.as_bytes().get(j), Some(b'+' | b'-')) {
                j += 1;
            }
            if matches!(self.src.as_bytes().get(j), Some(c) if c.is_ascii_digit()) {
                self.i = j;
                while matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
        }
        match self.src[start..self.i].parse() {
            Ok(n) => Ok(Expr::Number(n)),
            Err(_) => self.err(
                start,
                format!("malformed number `{}`", &self.src[start..self.i]),
            ),
        }
    }

    /// Quoted string; escapes are `\'` `\"` `\\`.
    fn string(&mut self, quote: u8) -> Result<Expr, ExprError> {
        let start = self.i;
        self.i += 1;
        let mut out = String::new();
        loop {
            match self.b() {
                None => return self.err(start, "unclosed string in expression"),
                Some(c) if c == quote => {
                    self.i += 1;
                    return Ok(Expr::Str(out));
                }
                Some(b'\\') => {
                    let at = self.i;
                    self.i += 1;
                    match self.b() {
                        Some(c @ (b'\'' | b'"' | b'\\')) => {
                            out.push(c as char);
                            self.i += 1;
                        }
                        Some(_) => {
                            let c = self.src[self.i..].chars().next().unwrap();
                            return self.err(at, format!("unknown escape `\\{c}` in string"));
                        }
                        None => return self.err(start, "unclosed string in expression"),
                    }
                }
                Some(_) => {
                    let c = self.src[self.i..].chars().next().unwrap();
                    out.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }
}

// -------------------------------------------------------------- evaluation

/// Name-resolution scope: loop variables layered over a root data map.
/// Lookup checks locals innermost-first, then the root map's keys; a missing
/// name is `null` (SPEC §9.4). The reserved root `ctx` never resolves here —
/// [`eval`] intercepts it before scope lookup, so it cannot be shadowed.
pub struct Scope<'a> {
    root: &'a Value,
    locals: Vec<(String, Value)>,
}

impl<'a> Scope<'a> {
    /// `root` is the render data; names resolve against its keys if it is a
    /// map (any other root value just means every name is `null`).
    pub fn new(root: &'a Value) -> Self {
        Scope {
            root,
            locals: Vec::new(),
        }
    }

    /// Binds a loop variable, shadowing outer bindings of the same name.
    pub fn push(&mut self, name: impl Into<String>, value: Value) {
        self.locals.push((name.into(), value));
    }

    /// Removes the innermost binding (matched with a preceding `push`).
    pub fn pop(&mut self) {
        self.locals.pop();
    }

    fn lookup(&self, name: &str) -> Option<&Value> {
        if let Some((_, v)) = self.locals.iter().rev().find(|(n, _)| n == name) {
            return Some(v);
        }
        match self.root {
            Value::Map(m) => m.iter().find(|(k, _)| k == name).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// Evaluates an expression (SPEC §9.4). `ctx` is the host-provided context
/// value, resolved for the name `ctx` in every scope, unshadowable.
pub fn eval(e: &Expr, scope: &Scope, ctx: &Value) -> Result<Value, RenderError> {
    match e {
        Expr::Null => Ok(Value::Null),
        Expr::Bool(b) => Ok(Value::Bool(*b)),
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::Str(s) => Ok(Value::Str(s.clone())),
        Expr::Name(n) => Ok(if n == "ctx" {
            ctx.clone()
        } else {
            scope.lookup(n).cloned().unwrap_or(Value::Null)
        }),
        Expr::Field(base, name) => Ok(match eval(base, scope, ctx)? {
            Value::Map(m) => m
                .into_iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .unwrap_or(Value::Null),
            _ => Value::Null,
        }),
        Expr::Index(base, idx) => {
            let b = eval(base, scope, ctx)?;
            let i = eval(idx, scope, ctx)?;
            Ok(match (b, i) {
                (Value::List(mut v), Value::Number(n)) => {
                    if n.fract() == 0.0 && n >= 0.0 && (n as usize) < v.len() {
                        v.swap_remove(n as usize)
                    } else {
                        Value::Null
                    }
                }
                (Value::Map(m), Value::Str(key)) => m
                    .into_iter()
                    .find(|(k, _)| *k == key)
                    .map(|(_, v)| v)
                    .unwrap_or(Value::Null),
                _ => Value::Null,
            })
        }
        Expr::Not(x) => Ok(Value::Bool(!eval(x, scope, ctx)?.truthy())),
        Expr::Neg(x) => match eval(x, scope, ctx)? {
            Value::Number(n) => Ok(Value::Number(-n)),
            v => Err(RenderError::new(format!(
                "unary `-` requires a number, got {}",
                v.describe()
            ))),
        },
        Expr::And(a, b) => {
            let va = eval(a, scope, ctx)?;
            if va.truthy() {
                eval(b, scope, ctx)
            } else {
                Ok(va)
            }
        }
        Expr::Or(a, b) => {
            let va = eval(a, scope, ctx)?;
            if va.truthy() {
                Ok(va)
            } else {
                eval(b, scope, ctx)
            }
        }
        Expr::Ternary(cond, then, els) => {
            if eval(cond, scope, ctx)?.truthy() {
                eval(then, scope, ctx)
            } else {
                eval(els, scope, ctx)
            }
        }
        Expr::Binary(op, a, b) => binary(*op, eval(a, scope, ctx)?, eval(b, scope, ctx)?),
    }
}

fn binary(op: BinOp, a: Value, b: Value) -> Result<Value, RenderError> {
    match op {
        BinOp::Eq => Ok(Value::Bool(deep_eq(&a, &b))),
        BinOp::Ne => Ok(Value::Bool(!deep_eq(&a, &b))),
        BinOp::Add => match (a, b) {
            (Value::Number(x), Value::Number(y)) => finite(BinOp::Add, x + y),
            (Value::Str(s), other) => Ok(Value::Str(s + &concat_operand(&other)?)),
            (other, Value::Str(s)) => Ok(Value::Str(concat_operand(&other)? + &s)),
            (a, b) => Err(RenderError::new(format!(
                "`+` requires two numbers or a string operand, got {} and {}",
                a.describe(),
                b.describe()
            ))),
        },
        _ => {
            let (Value::Number(x), Value::Number(y)) = (&a, &b) else {
                return Err(RenderError::new(format!(
                    "`{}` requires numbers, got {} and {}",
                    op.sym(),
                    a.describe(),
                    b.describe()
                )));
            };
            let (x, y) = (*x, *y);
            match op {
                BinOp::Lt => Ok(Value::Bool(x < y)),
                BinOp::Le => Ok(Value::Bool(x <= y)),
                BinOp::Gt => Ok(Value::Bool(x > y)),
                BinOp::Ge => Ok(Value::Bool(x >= y)),
                BinOp::Sub => finite(BinOp::Sub, x - y),
                BinOp::Mul => finite(BinOp::Mul, x * y),
                BinOp::Div if y == 0.0 => Err(RenderError::new("division by zero")),
                BinOp::Div => finite(BinOp::Div, x / y),
                BinOp::Mod if y == 0.0 => Err(RenderError::new("modulo by zero")),
                BinOp::Mod => finite(BinOp::Mod, x % y),
                BinOp::Eq | BinOp::Ne | BinOp::Add => unreachable!(),
            }
        }
    }
}

/// `+` string-concatenation operand: scalars stringify, lists/maps error
/// (SPEC §9.4).
fn concat_operand(v: &Value) -> Result<String, RenderError> {
    match v {
        Value::List(_) | Value::Map(_) => Err(RenderError::new(format!(
            "cannot use {} with `+`",
            v.describe()
        ))),
        _ => stringify(v),
    }
}

/// Arithmetic must stay finite — overflow to infinity would stringify
/// differently across backends, so it is an error instead.
fn finite(op: BinOp, n: f64) -> Result<Value, RenderError> {
    if n.is_finite() {
        Ok(Value::Number(n))
    } else {
        Err(RenderError::new(format!(
            "`{}` produced a non-finite number",
            op.sym()
        )))
    }
}
