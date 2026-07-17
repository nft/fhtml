//! Minimal strict JSON parser producing [`expr::Value`], for `--data`/`--ctx`
//! CLI input. Hand-rolled so the core stays zero-dependency: RFC 8259 only —
//! no comments, no trailing commas, no `NaN`/`Infinity`. Objects preserve key
//! order (the `for` statement iterates maps in insertion order); a duplicate
//! key keeps its first position but takes the last value.

use crate::expr::Value;
use std::fmt;

/// A JSON parse error with a 1-based position within the JSON text.
#[derive(Debug)]
pub struct JsonError {
    pub line: usize,
    pub col: usize,
    pub msg: String,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.msg)
    }
}

impl std::error::Error for JsonError {}

/// Parses one complete JSON document; trailing non-whitespace is an error.
pub fn parse(src: &str) -> Result<Value, JsonError> {
    let mut p = P { src, i: 0 };
    p.ws();
    let v = p.value()?;
    p.ws();
    if p.i < p.src.len() {
        return p.err("unexpected trailing content after JSON value");
    }
    Ok(v)
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
        while matches!(self.b(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.i += 1;
        }
    }

    fn err<T>(&self, msg: impl Into<String>) -> Result<T, JsonError> {
        self.err_at(self.i, msg)
    }

    fn err_at<T>(&self, offset: usize, msg: impl Into<String>) -> Result<T, JsonError> {
        let upto = &self.src[..offset.min(self.src.len())];
        let line = upto.matches('\n').count() + 1;
        let col = upto.chars().rev().take_while(|c| *c != '\n').count() + 1;
        Err(JsonError {
            line,
            col,
            msg: msg.into(),
        })
    }

    fn eat(&mut self, tok: &str) -> bool {
        if self.src[self.i..].starts_with(tok) {
            self.i += tok.len();
            true
        } else {
            false
        }
    }

    fn value(&mut self) -> Result<Value, JsonError> {
        match self.b() {
            None => self.err("expected a JSON value"),
            Some(b'n') if self.eat("null") => Ok(Value::Null),
            Some(b't') if self.eat("true") => Ok(Value::Bool(true)),
            Some(b'f') if self.eat("false") => Ok(Value::Bool(false)),
            Some(b'"') => Ok(Value::Str(self.string()?)),
            Some(b'[') => self.array(),
            Some(b'{') => self.object(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(_) => {
                let c = self.src[self.i..].chars().next().unwrap();
                self.err(format!("unexpected `{c}` — expected a JSON value"))
            }
        }
    }

    fn array(&mut self) -> Result<Value, JsonError> {
        self.i += 1; // [
        let mut items = Vec::new();
        self.ws();
        if self.eat("]") {
            return Ok(Value::List(items));
        }
        loop {
            items.push(self.value()?);
            self.ws();
            if self.eat("]") {
                return Ok(Value::List(items));
            }
            if !self.eat(",") {
                return self.err("expected `,` or `]` in array");
            }
            self.ws();
            if self.b() == Some(b']') {
                return self.err("trailing comma in array");
            }
        }
    }

    fn object(&mut self) -> Result<Value, JsonError> {
        self.i += 1; // {
        let mut map: Vec<(String, Value)> = Vec::new();
        self.ws();
        if self.eat("}") {
            return Ok(Value::Map(map));
        }
        loop {
            self.ws();
            if self.b() != Some(b'"') {
                return if self.b() == Some(b'}') {
                    self.err("trailing comma in object")
                } else {
                    self.err("expected a quoted object key")
                };
            }
            let key = self.string()?;
            self.ws();
            if !self.eat(":") {
                return self.err("expected `:` after object key");
            }
            self.ws();
            let value = self.value()?;
            match map.iter_mut().find(|(k, _)| *k == key) {
                Some(entry) => entry.1 = value,
                None => map.push((key, value)),
            }
            self.ws();
            if self.eat("}") {
                return Ok(Value::Map(map));
            }
            if !self.eat(",") {
                return self.err("expected `,` or `}` in object");
            }
        }
    }

    fn number(&mut self) -> Result<Value, JsonError> {
        let start = self.i;
        if self.b() == Some(b'-') {
            self.i += 1;
        }
        // integer part: `0` alone, or a nonzero digit run (no leading zeros)
        match self.b() {
            Some(b'0') => self.i += 1,
            Some(c) if c.is_ascii_digit() => {
                while matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                    self.i += 1;
                }
            }
            _ => return self.err_at(start, "malformed number"),
        }
        if matches!(self.b(), Some(c) if c.is_ascii_digit()) {
            return self.err_at(start, "numbers may not have leading zeros");
        }
        if self.b() == Some(b'.') {
            self.i += 1;
            if !matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                return self.err("expected a digit after `.`");
            }
            while matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.b(), Some(b'e' | b'E')) {
            self.i += 1;
            if matches!(self.b(), Some(b'+' | b'-')) {
                self.i += 1;
            }
            if !matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                return self.err("expected a digit in exponent");
            }
            while matches!(self.b(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        match self.src[start..self.i].parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(Value::Number(n)),
            _ => self.err_at(start, "number out of range"),
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        let start = self.i;
        self.i += 1; // opening quote
        let mut out = String::new();
        loop {
            match self.b() {
                None => return self.err_at(start, "unclosed string"),
                Some(b'"') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    let at = self.i;
                    self.i += 1;
                    match self.b() {
                        Some(b'"') => out.push('"'),
                        Some(b'\\') => out.push('\\'),
                        Some(b'/') => out.push('/'),
                        Some(b'b') => out.push('\u{0008}'),
                        Some(b'f') => out.push('\u{000C}'),
                        Some(b'n') => out.push('\n'),
                        Some(b'r') => out.push('\r'),
                        Some(b't') => out.push('\t'),
                        Some(b'u') => {
                            self.i += 1;
                            out.push(self.unicode_escape(at)?);
                            continue; // unicode_escape leaves `i` past the digits
                        }
                        Some(_) => {
                            let c = self.src[self.i..].chars().next().unwrap();
                            return self.err_at(at, format!("unknown escape `\\{c}`"));
                        }
                        None => return self.err_at(start, "unclosed string"),
                    }
                    self.i += 1;
                }
                Some(c) if c < 0x20 => {
                    return self.err("unescaped control character in string");
                }
                Some(_) => {
                    let c = self.src[self.i..].chars().next().unwrap();
                    out.push(c);
                    self.i += c.len_utf8();
                }
            }
        }
    }

    /// `\uXXXX`, called with `i` on the first hex digit. Handles UTF-16
    /// surrogate pairs (`😀` → 😀); a lone surrogate is an error.
    fn unicode_escape(&mut self, esc_at: usize) -> Result<char, JsonError> {
        let hi = self.hex4(esc_at)?;
        if (0xD800..0xDC00).contains(&hi) {
            if !self.eat("\\u") {
                return self.err_at(esc_at, "lone high surrogate in `\\u` escape");
            }
            let lo = self.hex4(esc_at)?;
            if !(0xDC00..0xE000).contains(&lo) {
                return self.err_at(esc_at, "invalid low surrogate in `\\u` escape");
            }
            let c = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
            return match char::from_u32(c) {
                Some(c) => Ok(c),
                None => self.err_at(esc_at, "invalid `\\u` escape"),
            };
        }
        match char::from_u32(hi) {
            Some(c) => Ok(c),
            None => self.err_at(esc_at, "lone surrogate in `\\u` escape"),
        }
    }

    fn hex4(&mut self, esc_at: usize) -> Result<u32, JsonError> {
        let digits = self.src.as_bytes().get(self.i..self.i + 4);
        let parsed = digits
            .and_then(|d| std::str::from_utf8(d).ok())
            .and_then(|d| u32::from_str_radix(d, 16).ok());
        match parsed {
            Some(n) => {
                self.i += 4;
                Ok(n)
            }
            None => self.err_at(esc_at, "`\\u` escape needs four hex digits"),
        }
    }
}

// ---- writing --------------------------------------------------------------

/// Serializes a [`Value`] as compact JSON. Finite integral numbers print as
/// JSON integers (`16`, not `16.0`) — protocol consumers (LSP positions and
/// ids, the WASM ABI envelope) require integer syntax; non-finite numbers
/// have no JSON spelling and print as `null`. Maps keep insertion order.
pub fn to_string(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut String) {
    use std::fmt::Write as _;
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                let _ = write!(out, "{}", *n as i64);
            } else if n.is_finite() {
                let _ = write!(out, "{n}");
            } else {
                out.push_str("null");
            }
        }
        Value::Str(s) => write_string(s, out),
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Map(pairs) => {
            out.push('{');
            for (i, (k, item)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_string(k, out);
                out.push(':');
                write_value(item, out);
            }
            out.push('}');
        }
    }
}

fn write_string(s: &str, out: &mut String) {
    use std::fmt::Write as _;
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::{parse, to_string};
    use crate::expr::Value;

    fn ok(src: &str) -> Value {
        parse(src).unwrap()
    }

    fn fails(src: &str) -> String {
        parse(src).unwrap_err().to_string()
    }

    #[test]
    fn scalars() {
        assert_eq!(ok("null"), Value::Null);
        assert_eq!(ok("true"), Value::Bool(true));
        assert_eq!(ok("false"), Value::Bool(false));
        assert_eq!(ok(" 42 "), Value::Number(42.0));
        assert_eq!(ok("\"hi\""), Value::Str("hi".into()));
    }

    #[test]
    fn number_forms() {
        assert_eq!(ok("-0"), Value::Number(-0.0));
        assert_eq!(ok("3.25"), Value::Number(3.25));
        assert_eq!(ok("-1.5e3"), Value::Number(-1500.0));
        assert_eq!(ok("1E+2"), Value::Number(100.0));
        assert_eq!(ok("0.5"), Value::Number(0.5));
        assert!(fails("01").contains("leading zeros"));
        assert!(fails("1.").contains("digit"));
        assert!(fails(".5").contains("expected a JSON value"));
        assert!(fails("+1").contains("expected a JSON value"));
        assert!(fails("1e").contains("digit"));
        assert!(fails("1e309").contains("out of range"));
        assert!(fails("NaN").contains("expected a JSON value"));
    }

    #[test]
    fn string_escapes() {
        assert_eq!(
            ok(r#""a\"b\\c\/d\n\tA""#),
            Value::Str("a\"b\\c/d\n\tA".into())
        );
        // nested escapes: a JSON string containing literal backslash-u
        assert_eq!(ok(r#""\\u0041""#), Value::Str("\\u0041".into()));
        // surrogate pair decodes to one code point; raw UTF-8 also passes
        assert_eq!(ok(r#""\uD83D\uDE00""#), Value::Str("😀".into()));
        assert_eq!(ok(r#""\u0041\u00e9""#), Value::Str("Aé".into()));
        assert_eq!(ok(r#""😀""#), Value::Str("😀".into()));
        assert!(fails(r#""\uD83D""#).contains("surrogate"));
        assert!(fails(r#""\uZZZZ""#).contains("four hex digits"));
        assert!(fails(r#""\x41""#).contains("unknown escape"));
        assert!(fails("\"a\nb\"").contains("control character"));
        assert!(fails("\"unclosed").contains("unclosed"));
    }

    #[test]
    fn arrays_and_objects() {
        assert_eq!(ok("[]"), Value::List(vec![]));
        assert_eq!(ok("{}"), Value::Map(vec![]));
        assert_eq!(
            ok(r#"[1, "two", [true], null]"#),
            Value::List(vec![
                Value::Number(1.0),
                Value::Str("two".into()),
                Value::List(vec![Value::Bool(true)]),
                Value::Null,
            ])
        );
        assert_eq!(
            ok(r#"{"b": 1, "a": {"nested": []}}"#),
            Value::Map(vec![
                ("b".into(), Value::Number(1.0)),
                (
                    "a".into(),
                    Value::Map(vec![("nested".into(), Value::List(vec![]))])
                ),
            ])
        );
    }

    #[test]
    fn objects_preserve_order_and_last_duplicate_wins() {
        assert_eq!(
            ok(r#"{"z": 1, "a": 2, "z": 3}"#),
            Value::Map(vec![
                ("z".into(), Value::Number(3.0)),
                ("a".into(), Value::Number(2.0)),
            ])
        );
    }

    #[test]
    fn strictness() {
        assert!(fails("[1, 2,]").contains("trailing comma"));
        assert!(fails(r#"{"a": 1,}"#).contains("trailing comma"));
        assert!(fails("[1 2]").contains("`,` or `]`"));
        assert!(fails(r#"{"a" 1}"#).contains("`:`"));
        assert!(fails(r#"{a: 1}"#).contains("quoted object key"));
        assert!(fails("'single'").contains("expected a JSON value"));
        assert!(fails("// comment\n1").contains("expected a JSON value"));
        assert!(fails("1 2").contains("trailing content"));
        assert!(fails("").contains("expected a JSON value"));
        assert!(fails("[1, ").contains("expected a JSON value"));
    }

    #[test]
    fn error_positions_are_line_and_column() {
        let e = parse("{\n  \"a\": 01\n}").unwrap_err();
        assert_eq!((e.line, e.col), (2, 8));
    }

    #[test]
    fn writer_round_trips_and_prints_integers() {
        let src = "{\"a\":[1,2.5,null,true],\"s\":\"x\\n\\\"y\\\"\",\"n\":{\"k\":16}}";
        let v = parse(src).unwrap();
        assert_eq!(to_string(&v), src);
        assert_eq!(to_string(&Value::Number(16.0)), "16");
        assert_eq!(to_string(&Value::Number(f64::NAN)), "null");
        assert_eq!(to_string(&Value::Str("\u{1}".into())), "\"\\u0001\"");
    }
}
