//! The fhtml WASM ABI: three exports,
//! one JSON envelope. `fh_call` takes a UTF-8 request like
//! `{"fn": "render", "files": {…}, "entry": "main.fhtml", …}` and returns
//! `{"ok": …}` or `{"err": {line?, col?, msg}}` — compile errors are data,
//! never traps. Both directions use the core's own JSON code
//! (`fhtml::json`); include resolution runs against a `MemVfs` built from
//! the request's file map, so no filesystem is ever touched.
//!
//! Memory contract (symmetric by construction): the host places the
//! request in a buffer from `fh_alloc` and frees it with `fh_dealloc`
//! after the call; the response buffer is allocated by the same routine
//! backing `fh_alloc` and is the host's to free via `fh_dealloc`. Layout
//! is `(len, align 1)` on both sides — nothing crosses the boundary with
//! different layout assumptions.

use std::alloc::{alloc, dealloc, Layout};
use std::path::Path;

use fhtml::{json, Analysis, Diag, FmtShorthand, MemVfs, Mode, Options, Span, Value, Vfs};

// ---- the ABI --------------------------------------------------------------

/// Allocates `len` bytes (align 1) for the host. `len == 0` returns null.
#[no_mangle]
pub extern "C" fn fh_alloc(len: u32) -> *mut u8 {
    if len == 0 {
        return std::ptr::null_mut();
    }
    // Align 1, so the layout is fully determined by `len` — `fh_dealloc`
    // can reconstruct it from the length alone.
    unsafe { alloc(Layout::from_size_align_unchecked(len as usize, 1)) }
}

/// Frees a buffer previously returned by [`fh_alloc`] (or by [`fh_call`] —
/// same allocation route) with its exact length.
///
/// # Safety
/// `ptr` must come from `fh_alloc`/`fh_call` with this exact `len`, and
/// must not be freed twice.
#[no_mangle]
pub unsafe extern "C" fn fh_dealloc(ptr: *mut u8, len: u32) {
    if !ptr.is_null() && len > 0 {
        dealloc(ptr, Layout::from_size_align_unchecked(len as usize, 1));
    }
}

/// One request in, one response out. Writes the response length through
/// `out_len` and returns the response pointer (host frees it via
/// [`fh_dealloc`]). All-u32 signature — no `i64` crosses the boundary.
///
/// # Safety
/// `ptr` must point to `len` readable bytes; `out_len` must point to a
/// writable 4-byte slot. The request buffer stays owned by the host.
#[no_mangle]
pub unsafe extern "C" fn fh_call(ptr: *const u8, len: u32, out_len: *mut u32) -> *mut u8 {
    let req = if ptr.is_null() || len == 0 {
        &[][..]
    } else {
        std::slice::from_raw_parts(ptr, len as usize)
    };
    let resp = match std::str::from_utf8(req) {
        Ok(text) => json::to_string(&response(text)),
        Err(_) => json::to_string(&err_msg("request is not valid UTF-8")),
    };
    let bytes = resp.as_bytes();
    let out = fh_alloc(bytes.len() as u32);
    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out, bytes.len());
    *out_len = bytes.len() as u32;
    out
}

// ---- dispatch -------------------------------------------------------------

fn response(req: &str) -> Value {
    let v = match json::parse(req) {
        Ok(v) => v,
        Err(e) => return err_msg(&format!("request is not valid JSON: {e}")),
    };
    let Some(name) = get_str(&v, "fn") else {
        return err_msg("request has no `fn` field");
    };
    match name {
        "render" => render(&v),
        "compileToJs" => compile_to_js(&v),
        "format" => format_src(&v),
        "analyze" => analyze(&v),
        "version" => ok(obj(vec![("version", str_val(env!("CARGO_PKG_VERSION")))])),
        other => err_msg(&format!(
            "unknown fn `{other}` — expected render, compileToJs, format, analyze, or version"
        )),
    }
}

fn render(v: &Value) -> Value {
    let (m, entry, src) = match vfs_and_entry(v) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let opts = match options(v) {
        Ok(o) => o,
        Err(e) => return e,
    };
    let data = get(v, "data").cloned().unwrap_or(Value::Null);
    let ctx = get(v, "ctx").cloned().unwrap_or(Value::Null);
    match fhtml::render_opts_vfs(&src, Some(Path::new(&entry)), &data, &ctx, &opts, &m) {
        Ok(out) => ok(obj(vec![
            ("html", str_val(&out.html)),
            ("warnings", warning_list(&out.warnings)),
        ])),
        Err(e) => err_compile(&e),
    }
}

fn compile_to_js(v: &Value) -> Value {
    let (m, entry, src) = match vfs_and_entry(v) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let opts = match options(v) {
        Ok(o) => o,
        Err(e) => return e,
    };
    match fhtml::compile_to_js_opts_vfs(&src, Some(Path::new(&entry)), &opts, &m) {
        Ok(out) => ok(obj(vec![
            ("js", str_val(&out.html)),
            ("warnings", warning_list(&out.warnings)),
        ])),
        Err(e) => err_compile(&e),
    }
}

fn format_src(v: &Value) -> Value {
    let Some(src) = get_str(v, "src") else {
        return err_msg("format needs a `src` string");
    };
    let shorthand = match get_str(v, "shorthand").unwrap_or("preserve") {
        "preserve" => FmtShorthand::Preserve,
        "expand" => FmtShorthand::Expand,
        "contract" => FmtShorthand::Contract,
        other => {
            return err_msg(&format!(
                "unknown shorthand mode `{other}` — expected preserve, expand, or contract"
            ))
        }
    };
    match fhtml::format_shorthand(src, shorthand) {
        Ok(out) => ok(obj(vec![("src", str_val(&out))])),
        Err(e) => err_compile(&e),
    }
}

fn analyze(v: &Value) -> Value {
    let (m, entry, src) = match vfs_and_entry(v) {
        Ok(t) => t,
        Err(e) => return e,
    };
    let a = fhtml::analyze_vfs(&src, Some(Path::new(&entry)), &m);
    ok(analysis_value(&a))
}

// ---- request pieces -------------------------------------------------------

/// The `{files, entry}` pair every source-taking fn shares. `entry` may be
/// omitted for a single-file map. The entry's source is read back through
/// the map so key spellings normalize the same way includes do.
fn vfs_and_entry(v: &Value) -> Result<(MemVfs, String, String), Value> {
    let Some(Value::Map(files)) = get(v, "files") else {
        return Err(err_msg("request needs a `files` object of name → source"));
    };
    let mut m = MemVfs::new();
    for (name, src) in files {
        let Value::Str(src) = src else {
            return Err(err_msg(&format!("files[\"{name}\"] is not a string")));
        };
        m.add(name, src.as_str());
    }
    let entry = match get_str(v, "entry") {
        Some(e) => e.to_string(),
        None if files.len() == 1 => files[0].0.clone(),
        None => {
            return Err(err_msg(
                "`entry` is required when `files` has more than one file",
            ))
        }
    };
    match m.read(Path::new(&entry)) {
        Ok(src) => Ok((m, entry, src)),
        Err(_) => Err(err_msg(&format!("entry `{entry}` is not in the file map"))),
    }
}

fn options(v: &Value) -> Result<Options, Value> {
    let mode = match get_str(v, "mode").unwrap_or("min") {
        "min" => Mode::Min,
        "pretty" => Mode::Pretty,
        other => {
            return Err(err_msg(&format!(
                "unknown mode `{other}` — expected min or pretty"
            )))
        }
    };
    Ok(Options {
        mode,
        ..Options::default()
    })
}

// ---- response building ----------------------------------------------------

fn ok(v: Value) -> Value {
    obj(vec![("ok", v)])
}

fn err_msg(msg: &str) -> Value {
    obj(vec![("err", obj(vec![("msg", str_val(msg))]))])
}

fn err_compile(e: &fhtml::Error) -> Value {
    obj(vec![(
        "err",
        obj(vec![
            ("line", num(e.line)),
            ("col", num(e.col)),
            ("msg", str_val(&e.msg)),
        ]),
    )])
}

/// Compiler warnings are `line:col: warning: msg` strings; the envelope
/// wraps them so the shape can grow structure without breaking callers.
fn warning_list(warnings: &[String]) -> Value {
    Value::List(
        warnings
            .iter()
            .map(|w| obj(vec![("msg", str_val(w))]))
            .collect(),
    )
}

fn span_value(s: &Span) -> Value {
    obj(vec![
        ("line", num(s.line)),
        ("col", num(s.col)),
        ("len", num(s.len)),
    ])
}

fn diag_value(d: &Diag) -> Value {
    obj(vec![
        ("line", num(d.line)),
        ("col", num(d.col)),
        ("len", num(d.len)),
        ("msg", str_val(&d.msg)),
    ])
}

fn analysis_value(a: &Analysis) -> Value {
    let defs = a
        .defs
        .iter()
        .map(|d| {
            let mut fields = vec![
                ("name", str_val(&d.name)),
                ("nameSpan", span_value(&d.name_span)),
                ("endLine", num(d.end_line)),
                (
                    "params",
                    Value::List(
                        d.params
                            .iter()
                            .map(|p| {
                                let mut f = vec![
                                    ("name", str_val(&p.name)),
                                    ("nameSpan", span_value(&p.name_span)),
                                ];
                                if let Some(def) = &p.default {
                                    f.push(("default", str_val(def)));
                                }
                                obj(f)
                            })
                            .collect(),
                    ),
                ),
            ];
            if let Some(file) = &d.file {
                fields.push(("file", str_val(&file.to_string_lossy())));
            }
            obj(fields)
        })
        .collect();
    let calls = a
        .calls
        .iter()
        .map(|c| {
            obj(vec![
                ("name", str_val(&c.name)),
                ("nameSpan", span_value(&c.name_span)),
                (
                    "args",
                    Value::List(
                        c.args
                            .iter()
                            .map(|arg| {
                                obj(vec![
                                    ("name", str_val(&arg.name)),
                                    ("span", span_value(&arg.span)),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ])
        })
        .collect();
    let includes = a
        .includes
        .iter()
        .map(|i| {
            let mut fields = vec![("path", str_val(&i.path)), ("span", span_value(&i.span))];
            if let Some(r) = &i.resolved {
                fields.push(("resolved", str_val(&r.to_string_lossy())));
            }
            obj(fields)
        })
        .collect();
    obj(vec![
        ("defs", Value::List(defs)),
        ("calls", Value::List(calls)),
        ("includes", Value::List(includes)),
        (
            "warnings",
            Value::List(a.warnings.iter().map(diag_value).collect()),
        ),
        (
            "error",
            a.error.as_ref().map(diag_value).unwrap_or(Value::Null),
        ),
    ])
}

// ---- Value helpers --------------------------------------------------------

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

#[cfg(test)]
mod tests {
    use super::response;
    use fhtml::json;

    fn call(req: &str) -> String {
        json::to_string(&response(req))
    }

    #[test]
    fn envelope_round_trips_natively() {
        // The dispatch layer is plain Rust — test it native, before the
        // wasm build even runs.
        let req = r#"{"fn":"render","files":{"lib.fhtml":"def badge(label)\n  span rounded \"{label}\"\n","main.fhtml":"include ./lib\n\ndiv\n  +badge(label={name})\n"},"entry":"main.fhtml","data":{"name":"hi"}}"#;
        let resp = call(req);
        assert!(resp.contains("\"ok\""), "got: {resp}");
        assert!(
            resp.contains("badge") || resp.contains("span"),
            "got: {resp}"
        );

        assert!(call("not json").contains("not valid JSON"));
        assert!(call("{\"fn\":\"nope\"}").contains("unknown fn `nope`"));
        assert!(call("{}").contains("no `fn` field"));
        let broken = r#"{"fn":"render","files":{"m.fhtml":"span \"unclosed\n"}}"#;
        let resp = call(broken);
        assert!(
            resp.contains("\"err\"") && resp.contains("\"line\""),
            "got: {resp}"
        );
        let v = call(r#"{"fn":"version"}"#);
        assert!(v.contains(env!("CARGO_PKG_VERSION")));
    }
}
