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

/// A range-valued member (`range`, `selectionRange`) of a diagnostic,
/// symbol, or edit, as ((line, char), (line, char)).
fn range_at(v: &Value, key: &str) -> ((f64, f64), (f64, f64)) {
    let range = get(v, key).expect(key);
    let at = |which: &str| {
        let pos = get(range, which).expect(which);
        (
            num(get(pos, "line").expect("line")),
            num(get(pos, "character").expect("character")),
        )
    };
    (at("start"), at("end"))
}

fn range_of(diag: &Value) -> ((f64, f64), (f64, f64)) {
    range_at(diag, "range")
}

fn as_list(v: &Value) -> &[Value] {
    match v {
        Value::List(l) => l,
        other => panic!("expected a list, got {other:?}"),
    }
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

// ---- formatting and document symbols ----------------------------

fn td_params(uri: &str) -> String {
    format!(r#"{{"textDocument":{{"uri":{}}}}}"#, js(uri))
}

#[test]
fn formatting_returns_one_whole_document_edit() {
    let mut lsp = Lsp::start();
    let uri = "untitled:Fmt-1";
    let src = "div\n    span   \"hi\"\n";
    lsp.open(uri, src);

    let reply = lsp.request(
        "textDocument/formatting",
        &format!(
            r#"{{"textDocument":{{"uri":{}}},"options":{{"tabSize":4,"insertSpaces":false}}}}"#,
            js(uri)
        ),
    );
    let edits = as_list(get(&reply, "result").expect("result"));
    assert_eq!(edits.len(), 1, "one whole-document edit");
    let expected = fhtml::format(src).unwrap();
    assert_ne!(expected, src);
    assert_eq!(get_str(&edits[0], "newText"), Some(expected.as_str()));
    // The edit replaces the entire document — client options (tabSize 4,
    // tabs) don't leak in; canonical fhtml is canonical.
    let ((sl, sc), (el, ec)) = range_of(&edits[0]);
    assert_eq!((sl, sc), (0.0, 0.0));
    assert_eq!((el, ec), (2.0, 0.0), "src has 2 lines + trailing newline");

    // Idempotence: the canonical text formats to no edits at all.
    lsp.change(uri, &expected);
    let reply = lsp.request("textDocument/formatting", &td_params(uri));
    assert_eq!(get(&reply, "result"), Some(&Value::List(vec![])));
    lsp.shutdown();
}

#[test]
fn formatting_declines_broken_and_unknown_documents() {
    let mut lsp = Lsp::start();
    // Parse error → null (diagnostics already show why), not a crash.
    lsp.open("untitled:Fmt-2", "div\n  span \"unclosed\n");
    let reply = lsp.request("textDocument/formatting", &td_params("untitled:Fmt-2"));
    assert_eq!(get(&reply, "result"), Some(&Value::Null));
    // Never-opened document → null.
    let reply = lsp.request(
        "textDocument/formatting",
        &td_params("untitled:never-opened"),
    );
    assert_eq!(get(&reply, "result"), Some(&Value::Null));
    lsp.shutdown();
}

#[test]
fn document_symbols_outline_defs_params_and_includes() {
    let mut lsp = Lsp::start();
    let uri = "untitled:Sym-1";
    // The include can't resolve on an untitled buffer (that error is
    // published as a diagnostic) — the outline must still list everything.
    let src = "include ./partials/lib\n\ndef card(title wide=false)\n  p \"{title}\"\n\ndiv\n  +card(title=\"x\")\n";
    lsp.open(uri, src);

    let reply = lsp.request("textDocument/documentSymbol", &td_params(uri));
    let syms = as_list(get(&reply, "result").expect("result"));
    assert_eq!(syms.len(), 2, "one include + one def: {syms:?}");

    let inc = &syms[0];
    assert_eq!(get_str(inc, "name"), Some("./partials/lib"));
    assert_eq!(num(get(inc, "kind").expect("kind")), 1.0); // File
    assert_eq!(range_of(inc), ((0.0, 8.0), (0.0, 22.0)));

    let def = &syms[1];
    assert_eq!(get_str(def, "name"), Some("card"));
    assert_eq!(num(get(def, "kind").expect("kind")), 12.0); // Function
    assert_eq!(get_str(def, "detail"), Some("(title wide=false)"));
    // Full range: the `def` line through its body; selection: the name.
    assert_eq!(range_of(def), ((2.0, 0.0), (3.0, 13.0)));
    assert_eq!(range_at(def, "selectionRange"), ((2.0, 4.0), (2.0, 8.0)));

    let params = as_list(get(def, "children").expect("children"));
    assert_eq!(params.len(), 2);
    assert_eq!(get_str(&params[0], "name"), Some("title"));
    assert_eq!(num(get(&params[0], "kind").expect("kind")), 13.0); // Variable
    assert_eq!(
        range_at(&params[0], "selectionRange"),
        ((2.0, 9.0), (2.0, 14.0))
    );
    assert_eq!(get_str(&params[1], "name"), Some("wide"));
    assert_eq!(
        range_at(&params[1], "selectionRange"),
        ((2.0, 15.0), (2.0, 19.0))
    );
    lsp.shutdown();
}

// ---- definition + completion ------------------------------------

fn pos_params(uri: &str, line: usize, character: usize) -> String {
    format!(
        r#"{{"textDocument":{{"uri":{}}},"position":{{"line":{line},"character":{character}}}}}"#,
        js(uri)
    )
}

fn labels(items: &[Value]) -> Vec<&str> {
    items.iter().filter_map(|i| get_str(i, "label")).collect()
}

/// Gate: call in A, def in included B — F12 jumps into
/// the included file; completion sees both files' components.
#[test]
fn definition_and_completion_across_a_two_file_include() {
    let f = Fixture::new();
    f.write(
        "partials/lib.fhtml",
        "def badge(label tone=1)\n  span \"{label}\"\n",
    );
    let main = f.write(
        "main.fhtml",
        "include ./partials/lib\n\ndef local(x)\n  p \"{x}\"\n\ndiv\n  +badge(label=\"hi\")\n  +local(x=1)\n",
    );
    let src = fs::read_to_string(&main).unwrap();
    let uri = file_uri(&main);
    let lib_uri = file_uri(&f.root.join("partials/lib.fhtml"));

    let mut lsp = Lsp::start();
    lsp.open(&uri, &src);

    // Definition on the `badge` call (line 7, inside the name) → the `def`
    // in the included file, with the name token's range there.
    let reply = lsp.request("textDocument/definition", &pos_params(&uri, 6, 5));
    let loc = get(&reply, "result").expect("result");
    assert_eq!(get_str(loc, "uri"), Some(lib_uri.as_str()));
    assert_eq!(range_of(loc), ((0.0, 4.0), (0.0, 9.0)));

    // Definition on the `local` call → the same-file def.
    let reply = lsp.request("textDocument/definition", &pos_params(&uri, 7, 4));
    let loc = get(&reply, "result").expect("result");
    assert_eq!(get_str(loc, "uri"), Some(uri.as_str()));
    assert_eq!(range_of(loc), ((2.0, 4.0), (2.0, 9.0)));

    // Definition on the include path → the included file itself.
    let reply = lsp.request("textDocument/definition", &pos_params(&uri, 0, 12));
    let loc = get(&reply, "result").expect("result");
    assert_eq!(get_str(loc, "uri"), Some(lib_uri.as_str()));
    assert_eq!(range_of(loc), ((0.0, 0.0), (0.0, 0.0)));

    // Definition elsewhere (plain element text) → null.
    let reply = lsp.request("textDocument/definition", &pos_params(&uri, 5, 1));
    assert_eq!(get(&reply, "result"), Some(&Value::Null));

    // Completion right after `+` lists both files' components.
    let reply = lsp.request("textDocument/completion", &pos_params(&uri, 6, 3));
    let items = as_list(get(&reply, "result").expect("result"));
    let names = labels(items);
    assert!(
        names.contains(&"badge") && names.contains(&"local"),
        "{names:?}"
    );
    let badge = items
        .iter()
        .find(|i| get_str(i, "label") == Some("badge"))
        .unwrap();
    assert_eq!(num(get(badge, "kind").expect("kind")), 3.0); // Function
    assert_eq!(get_str(badge, "detail"), Some("(label tone=1)"));

    // Completion inside `+badge(label="hi")`'s parens, before `)`: only the
    // unsupplied param remains.
    let reply = lsp.request("textDocument/completion", &pos_params(&uri, 6, 19));
    let items = as_list(get(&reply, "result").expect("result"));
    assert_eq!(labels(items), vec!["tone"], "label is already supplied");
    assert_eq!(get_str(&items[0], "insertText"), Some("tone="));

    // Mid-keystroke: the buffer breaks (`+b` being typed after an unclosed
    // string) — included components must still complete via the rescan.
    lsp.change(&uri, "include ./partials/lib\n\nspan \"unclosed\n\n+b");
    let reply = lsp.request("textDocument/completion", &pos_params(&uri, 4, 2));
    let items = as_list(get(&reply, "result").expect("result"));
    assert!(labels(items).contains(&"badge"), "{:?}", labels(items));
    lsp.shutdown();
}

#[test]
fn completion_at_line_start_offers_keywords_and_tags() {
    let mut lsp = Lsp::start();
    let uri = "untitled:Complete-1";
    lsp.open(uri, "div\n  \n");

    let reply = lsp.request("textDocument/completion", &pos_params(uri, 1, 2));
    let items = as_list(get(&reply, "result").expect("result"));
    let names = labels(items);
    for expected in [
        "if", "for", "def", "children", "include", "doctype", "div", "span", "section",
    ] {
        assert!(names.contains(&expected), "missing `{expected}`");
    }
    let kw = items
        .iter()
        .find(|i| get_str(i, "label") == Some("for"))
        .unwrap();
    assert_eq!(num(get(kw, "kind").expect("kind")), 14.0); // Keyword

    // No context (cursor inside quoted text) → empty list, not tag spam.
    lsp.change(uri, "div \"hi\"\n");
    let reply = lsp.request("textDocument/completion", &pos_params(uri, 0, 6));
    assert_eq!(get(&reply, "result"), Some(&Value::List(vec![])));
    lsp.shutdown();
}

// ---- the corpus gate ------------------------------------------------------

#[test]
fn lsp_formatting_matches_fmt_on_every_corpus_file() {
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
            lsp.open(&uri, &src);
            let reply = lsp.request("textDocument/formatting", &td_params(&uri));
            let expected = fhtml::format(&src).unwrap();
            let edits = as_list(get(&reply, "result").expect("result"));
            match edits {
                [] => assert_eq!(expected, src, "{}", path.display()),
                [edit] => {
                    assert_eq!(
                        get_str(edit, "newText"),
                        Some(expected.as_str()),
                        "{}",
                        path.display()
                    );
                    let ((sl, sc), _) = range_of(edit);
                    assert_eq!((sl, sc), (0.0, 0.0));
                }
                more => panic!(
                    "{}: expected at most one edit, got {more:?}",
                    path.display()
                ),
            }
            lsp.close(&uri);
            checked += 1;
        }
    }
    assert!(checked >= 49, "expected the full corpus, checked {checked}");
    lsp.shutdown();
}

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
