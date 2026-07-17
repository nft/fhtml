//! Tests for the `analyze()` tooling API.
//!
//! The gate: every reported span round-trips — slicing the source at
//! (line, col, len), counted in chars with indentation included (SPEC §11),
//! yields exactly the symbol's text. Checked on purpose-built fixtures and
//! swept over every corpus and site file.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use fhtml::{analyze, Analysis, Span};

/// Chars `col-1 .. col-1+len` of line `line` (1-based, physical).
fn slice(src: &str, s: &Span) -> String {
    let line = src
        .split('\n')
        .nth(s.line - 1)
        .unwrap_or("")
        .trim_end_matches('\r');
    line.chars().skip(s.col - 1).take(s.len).collect()
}

/// Every span in `a` slices back to its symbol's text. Defs from included
/// files slice against their own file's source.
fn assert_round_trip(src: &str, a: &Analysis) {
    for d in &a.defs {
        let owned;
        let owner = match &d.file {
            Some(f) => {
                owned = fs::read_to_string(f).unwrap();
                &owned
            }
            None => src,
        };
        assert_eq!(slice(owner, &d.name_span), d.name, "def `{}`", d.name);
        assert!(d.end_line >= d.name_span.line);
        for p in &d.params {
            // The def-name fallback (`\`-continued lists) never fires on
            // fixture/corpus files, so params must round-trip exactly too.
            assert_eq!(slice(owner, &p.name_span), p.name, "param `{}`", p.name);
        }
    }
    for c in &a.calls {
        assert_eq!(slice(src, &c.name_span), c.name, "call `{}`", c.name);
        for arg in &c.args {
            assert_eq!(slice(src, &arg.span), arg.name, "arg `{}`", arg.name);
        }
    }
    for i in &a.includes {
        assert_eq!(slice(src, &i.span), i.path, "include `{}`", i.path);
    }
}

// ---- fixtures on disk (includes need real files) --------------------------

static N: AtomicU32 = AtomicU32::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "fhtml-analyze-{}-{}",
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

    /// Canonical form (macOS temp dirs sit behind a symlink).
    fn canon(&self, rel: &str) -> PathBuf {
        fs::canonicalize(self.root.join(rel)).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

// ---- symbols and spans ----------------------------------------------------

const COMPONENTS: &str = r#"def card(title wide=false)
  . rounded-xl p-6 {wide ? 'col-span-2' : ''}
    h3 text-lg "{title}"
    children

div grid gap-4
  +card(title="Stats" wide=true)
    p text-sm "Body"
  ul
    li > span "x"
    +card(title="Two")
"#;

#[test]
fn def_and_call_spans_are_exact_and_physical() {
    let a = analyze(COMPONENTS, None);
    assert!(a.error.is_none());
    assert_round_trip(COMPONENTS, &a);

    assert_eq!(a.defs.len(), 1);
    let d = &a.defs[0];
    assert_eq!(d.name, "card");
    assert_eq!(
        d.name_span,
        Span {
            line: 1,
            col: 5,
            len: 4
        }
    );
    assert_eq!(d.end_line, 4);
    assert!(d.file.is_none());
    let params: Vec<(&str, Option<&str>)> = d
        .params
        .iter()
        .map(|p| (p.name.as_str(), p.default.as_deref()))
        .collect();
    assert_eq!(params, vec![("title", None), ("wide", Some("false"))]);
    // Param name tokens inside the parens: `def card(title wide=false)`.
    assert_eq!(
        d.params[0].name_span,
        Span {
            line: 1,
            col: 10,
            len: 5
        }
    );
    assert_eq!(
        d.params[1].name_span,
        Span {
            line: 1,
            col: 16,
            len: 4
        }
    );

    // Nested calls: columns count the indentation (SPEC §11).
    assert_eq!(a.calls.len(), 2);
    assert_eq!(
        a.calls[0].name_span,
        Span {
            line: 7,
            col: 4,
            len: 4
        }
    );
    let args: Vec<&str> = a.calls[0].args.iter().map(|x| x.name.as_str()).collect();
    assert_eq!(args, vec!["title", "wide"]);
    assert_eq!(
        a.calls[0].args[1].span,
        Span {
            line: 7,
            col: 23,
            len: 4
        }
    );
    assert_eq!(
        a.calls[1].name_span,
        Span {
            line: 11,
            col: 6,
            len: 4
        }
    );
}

#[test]
fn multibyte_text_keeps_char_columns() {
    // `✨🎉` are one char each in the column arithmetic (chars, not bytes;
    // UTF-16 conversion is the LSP transport's job).
    let src = "def chip(label tone=1)\n  span \"{label}\"\n\n+chip(label=\"✨🎉\" tone=2)\n";
    let a = analyze(src, None);
    assert!(a.error.is_none());
    assert_round_trip(src, &a);
    let tone = &a.calls[0].args[1];
    assert_eq!(
        tone.span,
        Span {
            line: 4,
            col: 18,
            len: 4
        }
    );
}

#[test]
fn warnings_are_structured_with_physical_columns() {
    let src = "div\n  span {\"bg-\" + color} \"chip\"\n";
    let a = analyze(src, None);
    assert!(a.error.is_none());
    assert_eq!(a.warnings.len(), 1);
    let w = &a.warnings[0];
    assert!(w.msg.contains("string concatenation"), "got: {}", w.msg);
    // The `{` of the interpolation, indentation included.
    assert_eq!((w.line, w.col), (2, 8));
    assert!(w.len >= 1);
}

#[test]
fn error_positions_count_indentation() {
    let src = "div\n  span \"unclosed\n";
    let a = analyze(src, None);
    let e = a.error.expect("parse error");
    // Content column 15 (past `"unclosed`) + 2 chars of indent.
    assert_eq!((e.line, e.col), (2, 17));
    assert!(e.len >= 1);
    assert!(e.msg.contains("unclosed string"), "got: {}", e.msg);
}

// ---- cross-file -----------------------------------------------------------

#[test]
fn included_defs_carry_their_file_and_positions() {
    let f = Fixture::new();
    f.write(
        "partials/lib.fhtml",
        "def badge(label)\n  span rounded \"{label}\"\n",
    );
    let main = f.write(
        "main.fhtml",
        "include ./partials/lib\n\ndiv\n  +badge(label=\"x\")\n",
    );
    let src = fs::read_to_string(&main).unwrap();
    let a = analyze(&src, Some(&main));
    assert!(a.error.is_none(), "got: {:?}", a.error);
    assert_round_trip(&src, &a);

    assert_eq!(a.includes.len(), 1);
    assert_eq!(a.includes[0].path, "./partials/lib");
    assert_eq!(
        a.includes[0].resolved.as_deref(),
        Some(f.canon("partials/lib.fhtml").as_path())
    );

    let badge = a.defs.iter().find(|d| d.name == "badge").expect("badge");
    assert_eq!(
        badge.file.as_deref(),
        Some(f.canon("partials/lib.fhtml").as_path())
    );
    // Positions are in the included file's own source.
    assert_eq!(
        badge.name_span,
        Span {
            line: 1,
            col: 5,
            len: 5
        }
    );
    assert_eq!(badge.end_line, 2);
}

#[test]
fn include_errors_match_compile_errors() {
    let f = Fixture::new();
    let main = f.write("main.fhtml", "include ./nope\n\np \"hi\"\n");
    let src = fs::read_to_string(&main).unwrap();
    let a = analyze(&src, Some(&main));
    let e = a.error.expect("resolution error");
    let compile = fhtml::deps_from(&src, Some(&main)).unwrap_err();
    assert_eq!(
        (e.line, e.col, &e.msg),
        (compile.line, compile.col, &compile.msg)
    );
    // The rest of the analysis is still there.
    assert_eq!(a.includes.len(), 1);
    assert!(a.includes[0].resolved.is_none());
}

#[test]
fn without_a_file_path_analysis_is_same_file_only() {
    let src = "include ./partials/lib\n\np \"hi\"\n";
    let a = analyze(src, None);
    // The stdin-has-no-base-path error, exactly as compile reports it.
    let e = a.error.expect("no base path error");
    assert!(e.msg.contains("no file path"), "got: {}", e.msg);
    assert_eq!(a.includes.len(), 1);
    assert!(a.includes[0].resolved.is_none());
}

// ---- best-effort on unparsable source -------------------------------------

#[test]
fn unparsable_source_still_lists_symbols() {
    let f = Fixture::new();
    f.write("head.fhtml", "def brand()\n  strong \"fhtml\"\n");
    let main = f.write(
        "main.fhtml",
        "def card(title wide=false)\n  p \"hi\"\n\ninclude ./head\n\n+card(title=\"x\")\n\nspan \"unclosed\n",
    );
    let src = fs::read_to_string(&main).unwrap();
    let a = analyze(&src, Some(&main));

    let e = a.error.as_ref().expect("parse error");
    assert_eq!(e.line, 8);

    let card = a.defs.iter().find(|d| d.name == "card").expect("card");
    assert_eq!(
        card.name_span,
        Span {
            line: 1,
            col: 5,
            len: 4
        }
    );
    let params: Vec<&str> = card.params.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(params, vec!["title", "wide"]);
    assert_eq!(card.params[1].default.as_deref(), Some("false"));
    assert_eq!(card.end_line, 2);

    assert_eq!(a.includes.len(), 1);
    assert_eq!(a.includes[0].path, "./head");
    assert!(a.includes[0].resolved.is_some());
    // Included files are usually intact while the buffer is mid-edit —
    // their defs stay available.
    let brand = a.defs.iter().find(|d| d.name == "brand").expect("brand");
    assert_eq!(brand.file.as_deref(), Some(f.canon("head.fhtml").as_path()));

    assert_eq!(a.calls.len(), 1);
    assert_eq!(a.calls[0].name, "card");
    assert_round_trip(&src, &a);
}

// ---- the corpus gate ------------------------------------------------------

#[test]
fn spans_round_trip_on_every_corpus_file() {
    let mut checked = 0;
    for dir in ["bench/out/fhtml", "site"] {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("fhtml") {
                continue;
            }
            let src = fs::read_to_string(&path).unwrap();
            let a = analyze(&src, Some(Path::new(&path)));
            assert!(a.error.is_none(), "{}: {:?}", path.display(), a.error);
            assert_round_trip(&src, &a);
            checked += 1;
        }
    }
    assert!(checked >= 49, "expected the full corpus, checked {checked}");
}
