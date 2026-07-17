//! End-to-end tests for `fhtml lsp`: spawn the
//! real binary and speak Content-Length-framed JSON-RPC over its stdio.
//!
//! The gate: the full lifecycle runs against every corpus file with zero
//! panics, and positions arrive 0-based in UTF-16 code units (emoji in class
//! text shift the `character` offset).

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};

use fhtml::{json, Value};

// ---- a tiny LSP client ----------------------------------------------------

struct Lsp {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    next_id: i64,
}

impl Lsp {
    /// Spawns `fhtml lsp` without any handshake.
    fn spawn() -> Lsp {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fhtml"))
            .arg("lsp")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn fhtml lsp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        Lsp {
            child,
            stdin,
            stdout,
            next_id: 0,
        }
    }

    /// Spawns and runs the `initialize` handshake.
    fn start() -> Lsp {
        let mut lsp = Lsp::spawn();
        let init = lsp.request("initialize", r#"{"capabilities":{}}"#);
        assert!(
            get(&init, "result").is_some(),
            "initialize failed: {init:?}"
        );
        lsp.notify("initialized", "{}");
        lsp
    }

    fn send(&mut self, body: &str) {
        write!(self.stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        self.stdin.flush().unwrap();
    }

    /// Reads one framed message.
    fn read_msg(&mut self) -> Value {
        let mut len: Option<usize> = None;
        loop {
            let mut line = String::new();
            assert!(
                self.stdout.read_line(&mut line).unwrap() > 0,
                "server closed its stdout mid-conversation"
            );
            let line = line.trim_end();
            if line.is_empty() {
                break;
            }
            if let Some(v) = line.strip_prefix("Content-Length:") {
                len = Some(v.trim().parse().unwrap());
            }
        }
        let mut body = vec![0u8; len.expect("Content-Length header")];
        self.stdout.read_exact(&mut body).unwrap();
        json::parse(std::str::from_utf8(&body).unwrap()).expect("well-formed JSON from the server")
    }

    /// Sends a request and reads until its response, dropping interleaved
    /// notifications.
    fn request(&mut self, method: &str, params: &str) -> Value {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#
        ));
        loop {
            let msg = self.read_msg();
            if let Some(Value::Number(n)) = get(&msg, "id") {
                assert_eq!(*n as i64, id, "response to an unexpected id");
                return msg;
            }
        }
    }

    fn notify(&mut self, method: &str, params: &str) {
        self.send(&format!(
            r#"{{"jsonrpc":"2.0","method":"{method}","params":{params}}}"#
        ));
    }

    /// Reads until a `publishDiagnostics` notification; returns (uri, list).
    fn diagnostics(&mut self) -> (String, Vec<Value>) {
        loop {
            let msg = self.read_msg();
            if get_str(&msg, "method") == Some("textDocument/publishDiagnostics") {
                let params = get(&msg, "params").expect("params");
                let uri = get_str(params, "uri").expect("uri").to_string();
                let diags = match get(params, "diagnostics") {
                    Some(Value::List(l)) => l.clone(),
                    other => panic!("diagnostics should be a list, got {other:?}"),
                };
                return (uri, diags);
            }
        }
    }

    /// `didOpen` and the diagnostics the server pushes for it.
    fn open(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.notify(
            "textDocument/didOpen",
            &format!(
                r#"{{"textDocument":{{"uri":{},"languageId":"fhtml","version":1,"text":{}}}}}"#,
                js(uri),
                js(text)
            ),
        );
        let (got, diags) = self.diagnostics();
        assert_eq!(got, uri);
        diags
    }

    /// Full-sync `didChange` and the refreshed diagnostics.
    fn change(&mut self, uri: &str, text: &str) -> Vec<Value> {
        self.notify(
            "textDocument/didChange",
            &format!(
                r#"{{"textDocument":{{"uri":{},"version":2}},"contentChanges":[{{"text":{}}}]}}"#,
                js(uri),
                js(text)
            ),
        );
        let (got, diags) = self.diagnostics();
        assert_eq!(got, uri);
        diags
    }

    fn close(&mut self, uri: &str) -> Vec<Value> {
        self.notify(
            "textDocument/didClose",
            &format!(r#"{{"textDocument":{{"uri":{}}}}}"#, js(uri)),
        );
        let (got, diags) = self.diagnostics();
        assert_eq!(got, uri);
        diags
    }

    /// `shutdown` → `exit` → the process ends with status 0.
    fn shutdown(mut self) {
        let reply = self.request("shutdown", "null");
        assert_eq!(get(&reply, "result"), Some(&Value::Null));
        self.notify("exit", "null");
        let status = self.child.wait().unwrap();
        assert!(
            status.success(),
            "exit after shutdown should be 0: {status}"
        );
    }
}

/// JSON string literal for `s`.
fn js(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Member of a JSON object (tests only need one level at a time).
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

fn num(v: &Value) -> f64 {
    match v {
        Value::Number(n) => *n,
        other => panic!("expected a number, got {other:?}"),
    }
}

/// `range.start`/`range.end` of a diagnostic as ((line, char), (line, char)).
fn range_of(diag: &Value) -> ((f64, f64), (f64, f64)) {
    let range = get(diag, "range").expect("range");
    let at = |which: &str| {
        let pos = get(range, which).expect(which);
        (
            num(get(pos, "line").expect("line")),
            num(get(pos, "character").expect("character")),
        )
    };
    (at("start"), at("end"))
}

fn severity(diag: &Value) -> f64 {
    num(get(diag, "severity").expect("severity"))
}

fn message(diag: &Value) -> &str {
    get_str(diag, "message").expect("message")
}

// ---- fixtures on disk (URI→path resolution needs real files) --------------

static N: AtomicU32 = AtomicU32::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "fhtml-lsp-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).unwrap();
        Fixture { root }
    }

    fn write(&self, rel: &str, src: &str) -> PathBuf {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, src).unwrap();
        path
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn file_uri(path: &Path) -> String {
    format!("file://{}", fs::canonicalize(path).unwrap().display())
}

// ---- lifecycle and capabilities -------------------------------------------

#[test]
fn initialize_advertises_the_v1_capabilities() {
    let mut lsp = Lsp::spawn();
    let init = lsp.request("initialize", r#"{"capabilities":{}}"#);
    let result = get(&init, "result").expect("result");
    let caps = get(result, "capabilities").expect("capabilities");

    // 1 = TextDocumentSyncKind.Full — the stateless whole-document contract.
    assert_eq!(num(get(caps, "textDocumentSync").expect("sync")), 1.0);
    for provider in [
        "documentFormattingProvider",
        "documentSymbolProvider",
        "definitionProvider",
    ] {
        assert_eq!(
            get(caps, provider),
            Some(&Value::Bool(true)),
            "{provider} should be advertised"
        );
    }
    let completion = get(caps, "completionProvider").expect("completionProvider");
    assert_eq!(
        get(completion, "triggerCharacters"),
        Some(&Value::List(vec![
            Value::Str("+".into()),
            Value::Str("(".into())
        ]))
    );
    assert_eq!(
        get(result, "serverInfo").and_then(|s| get_str(s, "name")),
        Some("fhtml")
    );

    lsp.notify("initialized", "{}");
    lsp.shutdown();
}

#[test]
fn diagnostics_follow_open_change_close() {
    let mut lsp = Lsp::start();
    let uri = "file:///no/such/dir/scratch.fhtml";

    let diags = lsp.open(uri, "div\n  span \"unclosed\n");
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(severity(d), 1.0);
    assert!(
        message(d).contains("unclosed string"),
        "got: {}",
        message(d)
    );
    // Compiler position 2:17 (1-based chars, SPEC §11) → 0-based (1, 16),
    // clamped to the caret at end of the 16-char line.
    assert_eq!(range_of(d), ((1.0, 16.0), (1.0, 16.0)));

    let diags = lsp.change(uri, "div\n  span \"closed\"\n");
    assert!(diags.is_empty(), "fixed file should have no diagnostics");

    let diags = lsp.close(uri);
    assert!(diags.is_empty(), "close should clear diagnostics");
    lsp.shutdown();
}

#[test]
fn warning_positions_are_utf16_code_units() {
    let mut lsp = Lsp::start();
    let line = r#"  span x-🚀🎉 {"bg-" + tone} "chip""#;
    let src = format!("div\n{line}\n");

    let diags = lsp.open("untitled:Untitled-1", &src);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(severity(d), 2.0, "the concat lint is a warning");
    assert!(message(d).contains("concatenation"), "got: {}", message(d));

    // The diagnostic points at `{` — before it sit 2 astral-plane emoji,
    // each 1 char but 2 UTF-16 units, so the wire offset exceeds the char
    // offset by exactly 2.
    let char_col = line.chars().position(|c| c == '{').unwrap();
    let utf16_col: usize = line.chars().take(char_col).map(|c| c.len_utf16()).sum();
    assert_eq!(utf16_col, char_col + 2);
    let ((l, c), _) = range_of(d);
    assert_eq!((l, c), (1.0, utf16_col as f64));
    lsp.shutdown();
}

#[test]
fn include_errors_resolve_through_percent_encoded_uris() {
    let f = Fixture::new();
    let main = f.write("my dir/main.fhtml", "include ./nope\n\np \"hi\"\n");
    let src = fs::read_to_string(&main).unwrap();
    let uri = file_uri(&main).replace(' ', "%20");

    let mut lsp = Lsp::start();
    let diags = lsp.open(&uri, &src);
    assert_eq!(diags.len(), 1);
    let d = &diags[0];
    assert_eq!(severity(d), 1.0);
    assert!(
        message(d).contains("include") && message(d).contains("nope"),
        "expected the missing-include error, got: {}",
        message(d)
    );
    lsp.shutdown();
}

#[test]
fn untitled_buffers_analyze_without_a_path() {
    let mut lsp = Lsp::start();
    let diags = lsp.open("untitled:Untitled-2", "include ./partials/lib\n");
    assert_eq!(diags.len(), 1);
    assert!(
        message(&diags[0]).contains("no file path"),
        "got: {}",
        message(&diags[0])
    );
    lsp.shutdown();
}

#[test]
fn unknown_traffic_never_crashes_the_server() {
    let mut lsp = Lsp::start();

    // Unknown request → MethodNotFound, with the request's id.
    let reply = lsp.request("textDocument/hover", r#"{"position":{}}"#);
    let err = get(&reply, "error").expect("error reply");
    assert_eq!(num(get(err, "code").expect("code")), -32601.0);

    // Unknown notification → ignored; the server stays responsive.
    lsp.notify("$/setTrace", r#"{"value":"off"}"#);
    let diags = lsp.open("untitled:Untitled-3", "p \"ok\"\n");
    assert!(diags.is_empty());
    lsp.shutdown();
}

// ---- the corpus gate ------------------------------------------------------

#[test]
fn full_lifecycle_over_every_corpus_file() {
    let mut lsp = Lsp::start();
    let mut checked = 0;
    for dir in ["bench/out/fhtml", "site"] {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("fhtml") {
                continue;
            }
            let src = fs::read_to_string(&path).unwrap();
            let uri = file_uri(&path);
            let diags = lsp.open(&uri, &src);
            let errors: Vec<&Value> = diags.iter().filter(|d| severity(d) == 1.0).collect();
            assert!(errors.is_empty(), "{}: {errors:?}", path.display());
            let diags = lsp.close(&uri);
            assert!(diags.is_empty());
            checked += 1;
        }
    }
    assert!(checked >= 49, "expected the full corpus, checked {checked}");
    lsp.shutdown();
}
