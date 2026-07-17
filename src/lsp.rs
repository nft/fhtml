//! `fhtml lsp` — a zero-dependency Language Server.
//!
//! Content-Length-framed JSON-RPC over stdio, single-threaded, stateless:
//! full-document sync with a whole-document `analyze()` per change —
//! `.fhtml` files are small by design, so there is no incremental state to
//! get wrong. Reads parse through [`fhtml::json`]; writes go through the
//! hand-rolled serializer below, which prints integral numbers without a
//! fraction (LSP positions must be JSON integers).
//!
//! Positions: the compiler reports 1-based lines and 1-based *character*
//! columns counted from the physical line start (SPEC §11); the wire wants
//! 0-based lines and UTF-16 code-unit offsets. The conversion happens here,
//! at the transport boundary, and nowhere else.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::process::exit;

use fhtml::{analyze, Value};

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;

/// Serves LSP over stdin/stdout until the client sends `exit` (which
/// terminates the process directly) or closes the stream.
pub fn run() -> Result<(), String> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut server = Server {
        out: stdout.lock(),
        docs: HashMap::new(),
        shutdown: false,
    };
    let mut reader = stdin.lock();
    loop {
        match read_message(&mut reader)? {
            Some(text) => server.dispatch(&text),
            // The protocol ends with the `exit` notification; a client that
            // just drops the pipe gets the LSP-mandated error exit.
            None => return Err("fhtml lsp: client closed the stream without `exit`".to_string()),
        }
    }
}

/// Reads one Content-Length-framed message body. `None` on a clean EOF at a
/// message boundary.
fn read_message(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut content_length: Option<usize> = None;
    let mut first = true;
    loop {
        let mut line = Vec::new();
        let n = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| format!("fhtml lsp: read error: {e}"))?;
        if n == 0 {
            if first && content_length.is_none() {
                return Ok(None);
            }
            return Err("fhtml lsp: EOF inside a message header".to_string());
        }
        first = false;
        while matches!(line.last(), Some(b'\n' | b'\r')) {
            line.pop();
        }
        if line.is_empty() {
            break; // end of headers
        }
        let header = String::from_utf8_lossy(&line);
        if let Some((name, value)) = header.split_once(':') {
            if name.trim().eq_ignore_ascii_case("Content-Length") {
                let len = value
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| format!("fhtml lsp: bad Content-Length `{}`", value.trim()))?;
                content_length = Some(len);
            }
            // Content-Type and anything else: ignored.
        }
    }
    let len = content_length.ok_or("fhtml lsp: message without Content-Length")?;
    let mut body = vec![0u8; len];
    reader
        .read_exact(&mut body)
        .map_err(|e| format!("fhtml lsp: read error: {e}"))?;
    String::from_utf8(body).map(Some).map_err(|_| {
        "fhtml lsp: message body is not UTF-8 (only utf-8 content is supported)".to_string()
    })
}

struct Server<W: Write> {
    out: W,
    /// Current text of every open document, by URI — the full-sync state.
    docs: HashMap<String, String>,
    shutdown: bool,
}

impl<W: Write> Server<W> {
    fn dispatch(&mut self, text: &str) {
        let msg = match fhtml::json::parse(text) {
            Ok(v) => v,
            Err(e) => {
                self.respond_err(&Value::Null, PARSE_ERROR, &format!("parse error: {e}"));
                return;
            }
        };
        let params = get(&msg, "params").unwrap_or(&Value::Null);
        match (get_str(&msg, "method"), get(&msg, "id")) {
            (Some(method), Some(id)) => self.request(method, id, params),
            (Some(method), None) => self.notification(method, params),
            (None, Some(id)) => self.respond_err(id, INVALID_REQUEST, "message has no `method`"),
            (None, None) => {} // unaddressable garbage — nothing to answer
        }
    }

    fn request(&mut self, method: &str, id: &Value, params: &Value) {
        match method {
            "initialize" => self.respond(id, initialize_result()),
            "textDocument/formatting" => self.formatting(id, params),
            "textDocument/documentSymbol" => self.document_symbol(id, params),
            "shutdown" => {
                self.shutdown = true;
                self.respond(id, Value::Null);
            }
            // Every unimplemented or unknown request — never crash on
            // unexpected traffic.
            _ => self.respond_err(
                id,
                METHOD_NOT_FOUND,
                &format!("method not found: `{method}`"),
            ),
        }
    }

    fn notification(&mut self, method: &str, params: &Value) {
        match method {
            "initialized" => {}
            "exit" => exit(if self.shutdown { 0 } else { 1 }),
            "textDocument/didOpen" => {
                let doc = get(params, "textDocument").unwrap_or(&Value::Null);
                if let (Some(uri), Some(text)) = (get_str(doc, "uri"), get_str(doc, "text")) {
                    let uri = uri.to_string();
                    self.docs.insert(uri.clone(), text.to_string());
                    self.publish(&uri);
                }
            }
            "textDocument/didChange" => {
                let uri = get(params, "textDocument")
                    .and_then(|d| get_str(d, "uri"))
                    .map(str::to_string);
                // Full sync: each change carries the whole document; the
                // last one wins.
                let text = match get(params, "contentChanges") {
                    Some(Value::List(changes)) => changes
                        .iter()
                        .rev()
                        .find_map(|c| get_str(c, "text"))
                        .map(str::to_string),
                    _ => None,
                };
                if let (Some(uri), Some(text)) = (uri, text) {
                    self.docs.insert(uri.clone(), text);
                    self.publish(&uri);
                }
            }
            "textDocument/didClose" => {
                let uri = get(params, "textDocument")
                    .and_then(|d| get_str(d, "uri"))
                    .map(str::to_string);
                if let Some(uri) = uri {
                    self.docs.remove(&uri);
                    self.publish(&uri); // clear the file's diagnostics
                }
            }
            _ => {} // unknown notifications (incl. `$/…`): ignored
        }
    }

    /// `analyze` the document and push `textDocument/publishDiagnostics`:
    /// the parse/resolve error at severity Error, warnings (including the
    /// dynamic-class lint) at Warning. A closed document publishes an empty
    /// list, clearing its squiggles.
    fn publish(&mut self, uri: &str) {
        let diags = match self.docs.get(uri) {
            Some(text) => {
                let path = uri_to_path(uri);
                let a = analyze(text, path.as_deref());
                let mut diags = Vec::new();
                if let Some(e) = &a.error {
                    diags.push(diagnostic(text, e, 1));
                }
                for w in &a.warnings {
                    diags.push(diagnostic(text, w, 2));
                }
                diags
            }
            None => Vec::new(),
        };
        self.notify(
            "textDocument/publishDiagnostics",
            obj(vec![
                ("uri", str_val(uri)),
                ("diagnostics", Value::List(diags)),
            ]),
        );
    }

    /// URI of `params.textDocument`, if that document is open.
    fn doc_text(&self, params: &Value) -> Option<(String, &String)> {
        let uri = get(params, "textDocument").and_then(|d| get_str(d, "uri"))?;
        Some((uri.to_string(), self.docs.get(uri)?))
    }

    /// `textDocument/formatting`: whole-document reformat through
    /// [`fhtml::format`] — byte-identical to `fhtml fmt`. One edit spanning
    /// the entire document, an empty list when already canonical, and null
    /// when the source doesn't format (parse error — diagnostics already
    /// show why). Client formatting options are ignored: canonical fhtml is
    /// 2-space indented by definition.
    fn formatting(&mut self, id: &Value, params: &Value) {
        let Some((_, text)) = self.doc_text(params) else {
            self.respond(id, Value::Null);
            return;
        };
        let result = match fhtml::format(text) {
            Err(_) => Value::Null,
            Ok(formatted) if formatted == *text => Value::List(Vec::new()),
            Ok(formatted) => {
                let (line, character) = end_of(text);
                Value::List(vec![obj(vec![
                    (
                        "range",
                        obj(vec![
                            ("start", position(0, 0)),
                            ("end", position(line, character)),
                        ]),
                    ),
                    ("newText", str_val(&formatted)),
                ])])
            }
        };
        self.respond(id, result);
    }

    /// `textDocument/documentSymbol`: one symbol per `def` (kind Function,
    /// params as Variable children) plus `include` targets (kind File).
    /// Analyzed without a path — the outline is same-file by definition —
    /// and symbols survive parse errors via analyze's rescan.
    fn document_symbol(&mut self, id: &Value, params: &Value) {
        let Some((_, text)) = self.doc_text(params) else {
            self.respond(id, Value::Null);
            return;
        };
        let a = analyze(text, None);
        let mut symbols: Vec<(usize, Value)> = Vec::new();
        for inc in &a.includes {
            let range = range_value(text, &inc.span);
            symbols.push((
                inc.span.line,
                obj(vec![
                    ("name", str_val(&inc.path)),
                    ("kind", num(1)), // File
                    ("range", range.clone()),
                    ("selectionRange", range),
                    ("children", Value::List(Vec::new())),
                ]),
            ));
        }
        for d in &a.defs {
            let children: Vec<Value> = d
                .params
                .iter()
                .map(|p| {
                    let range = range_value(text, &p.name_span);
                    obj(vec![
                        ("name", str_val(&p.name)),
                        ("kind", num(13)), // Variable
                        ("range", range.clone()),
                        ("selectionRange", range),
                        ("children", Value::List(Vec::new())),
                    ])
                })
                .collect();
            let sig: Vec<String> = d
                .params
                .iter()
                .map(|p| match &p.default {
                    Some(v) => format!("{}={v}", p.name),
                    None => p.name.clone(),
                })
                .collect();
            // The whole definition block, def line through body end.
            let range = obj(vec![
                ("start", position(d.name_span.line - 1, 0)),
                (
                    "end",
                    position(d.end_line - 1, line_utf16_len(text, d.end_line)),
                ),
            ]);
            symbols.push((
                d.name_span.line,
                obj(vec![
                    ("name", str_val(&d.name)),
                    ("detail", str_val(&format!("({})", sig.join(" ")))),
                    ("kind", num(12)), // Function
                    ("range", range),
                    ("selectionRange", range_value(text, &d.name_span)),
                    ("children", Value::List(children)),
                ]),
            ));
        }
        symbols.sort_by_key(|(line, _)| *line);
        self.respond(
            id,
            Value::List(symbols.into_iter().map(|(_, s)| s).collect()),
        );
    }

    fn respond(&mut self, id: &Value, result: Value) {
        self.send(obj(vec![
            ("jsonrpc", str_val("2.0")),
            ("id", id.clone()),
            ("result", result),
        ]));
    }

    fn respond_err(&mut self, id: &Value, code: i64, message: &str) {
        self.send(obj(vec![
            ("jsonrpc", str_val("2.0")),
            ("id", id.clone()),
            (
                "error",
                obj(vec![
                    ("code", Value::Number(code as f64)),
                    ("message", str_val(message)),
                ]),
            ),
        ]));
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(obj(vec![
            ("jsonrpc", str_val("2.0")),
            ("method", str_val(method)),
            ("params", params),
        ]));
    }

    fn send(&mut self, msg: Value) {
        let body = to_json(&msg);
        // Content-Length counts bytes; `String::len` is exactly that.
        let _ = write!(self.out, "Content-Length: {}\r\n\r\n{body}", body.len());
        let _ = self.out.flush();
    }
}

fn initialize_result() -> Value {
    obj(vec![
        (
            "capabilities",
            obj(vec![
                // 1 = TextDocumentSyncKind.Full
                ("textDocumentSync", num(1)),
                ("documentFormattingProvider", Value::Bool(true)),
                ("documentSymbolProvider", Value::Bool(true)),
                ("definitionProvider", Value::Bool(true)),
                (
                    "completionProvider",
                    obj(vec![(
                        "triggerCharacters",
                        Value::List(vec![str_val("+"), str_val("(")]),
                    )]),
                ),
            ]),
        ),
        (
            "serverInfo",
            obj(vec![
                ("name", str_val("fhtml")),
                ("version", str_val(env!("CARGO_PKG_VERSION"))),
            ]),
        ),
    ])
}

/// One LSP `Diagnostic` from an analyze [`fhtml::Diag`], positions converted
/// to 0-based UTF-16 against the document text.
fn diagnostic(text: &str, d: &fhtml::Diag, severity: usize) -> Value {
    obj(vec![
        ("range", range(text, d.line, d.col, d.len)),
        ("severity", num(severity)),
        ("source", str_val("fhtml")),
        ("message", str_val(&d.msg)),
    ])
}

/// An LSP `Range` from a compiler span (1-based char columns, SPEC §11),
/// converted to 0-based UTF-16 against the document text.
fn range_value(text: &str, span: &fhtml::Span) -> Value {
    range(text, span.line, span.col, span.len)
}

fn range(text: &str, line: usize, col: usize, len: usize) -> Value {
    let line_text = source_line(text, line);
    let chars = line_text.chars().count();
    // Clamp: a position one past EOL (or past a `\`-joined continuation,
    // SPEC §11) collapses to a caret at the line end.
    let start = (col - 1).min(chars);
    let end = (col - 1 + len).min(chars);
    let utf16 =
        |upto: usize| -> usize { line_text.chars().take(upto).map(|c| c.len_utf16()).sum() };
    obj(vec![
        ("start", position(line - 1, utf16(start))),
        ("end", position(line - 1, utf16(end))),
    ])
}

fn source_line(text: &str, line: usize) -> &str {
    text.split('\n')
        .nth(line - 1)
        .unwrap_or("")
        .trim_end_matches('\r')
}

/// UTF-16 length of a 1-based source line.
fn line_utf16_len(text: &str, line: usize) -> usize {
    source_line(text, line).chars().map(|c| c.len_utf16()).sum()
}

/// 0-based UTF-16 position of the very end of the document.
fn end_of(text: &str) -> (usize, usize) {
    let lines = text.split('\n').count(); // >= 1 even for ""
    let last: usize = text
        .split('\n')
        .next_back()
        .unwrap_or("")
        .chars()
        .map(|c| c.len_utf16())
        .sum();
    (lines - 1, last)
}

fn position(line: usize, character: usize) -> Value {
    obj(vec![("line", num(line)), ("character", num(character))])
}

/// `file://` URI → filesystem path (percent-decoded, authority dropped).
/// Any other scheme (`untitled:` buffers) analyzes without a path —
/// same-file only, exactly like stdin.
fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let path = if rest.starts_with('/') {
        rest
    } else {
        // e.g. `file://localhost/x` — skip the authority
        &rest[rest.find('/')?..]
    };
    Some(PathBuf::from(percent_decode(path)))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(hi), Some(lo)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((hi * 16 + lo) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

// ---- JSON building and writing --------------------------------------------

fn obj(pairs: Vec<(&str, Value)>) -> Value {
    Value::Map(pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn str_val(s: &str) -> Value {
    Value::Str(s.to_string())
}

fn num(n: usize) -> Value {
    Value::Number(n as f64)
}

fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    match v {
        Value::Map(m) => m.iter().find(|(k, _)| k == key).map(|(_, v)| v),
        _ => None,
    }
}

fn get_str<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    match get(v, key) {
        Some(Value::Str(s)) => Some(s),
        _ => None,
    }
}

fn to_json(v: &Value) -> String {
    let mut out = String::new();
    write_json(v, &mut out);
    out
}

fn write_json(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => {
            // LSP positions and ids must be integers, not `16.0`.
            if n.is_finite() && n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
                let _ = write!(out, "{}", *n as i64);
            } else if n.is_finite() {
                let _ = write!(out, "{n}");
            } else {
                out.push_str("null");
            }
        }
        Value::Str(s) => write_json_string(s, out),
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(item, out);
            }
            out.push(']');
        }
        Value::Map(pairs) => {
            out.push('{');
            for (i, (k, item)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json_string(k, out);
                out.push(':');
                write_json(item, out);
            }
            out.push('}');
        }
    }
}

fn write_json_string(s: &str, out: &mut String) {
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
