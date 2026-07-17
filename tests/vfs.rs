//! Tests for the VFS seam.
//!
//! The gate: every multi-file construct — include chains, cross-file defs,
//! `..` relative paths, per-file `#!shorthand` scoping — behaves
//! byte-identically through a `MemVfs` and the disk, output and error text
//! alike (mem error text differs only in the path prefix and the
//! io-message tail).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use fhtml::{
    analyze, analyze_vfs, deps_from, deps_vfs, render_opts_from, render_opts_vfs, MemVfs, Mode,
    Options, Value,
};

fn opts(mode: Mode) -> Options {
    Options {
        mode,
        ..Options::default()
    }
}

// ---- fixtures -------------------------------------------------------------

static N: AtomicU32 = AtomicU32::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Fixture {
        let root = std::env::temp_dir().join(format!(
            "fhtml-vfs-{}-{}",
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

/// The same file set on disk (under a tempdir) and in a `MemVfs` (under its
/// relative names).
fn both(files: &[(&str, &str)]) -> (Fixture, MemVfs) {
    let f = Fixture::new();
    let mut m = MemVfs::new();
    for (rel, src) in files {
        f.write(rel, src);
        m.add(rel, *src);
    }
    (f, m)
}

/// A multi-file layout exercising a nested chain, a `..` path, cross-file
/// defs with `children`, and `#!shorthand` scoped to one included file.
const LAYOUT: &[(&str, &str)] = &[
    (
        "main.fhtml",
        "include ./partials/lib\ninclude ./partials/deep/head\n\ndiv grid\n  +card(title=\"Stats\")\n    p \"{ctx.who}\"\n  +brand()\n",
    ),
    (
        "partials/lib.fhtml",
        "def card(title wide=false)\n  . rounded-xl {wide ? 'col-span-2' : ''}\n    h3 \"{title}\"\n    children\n",
    ),
    (
        "partials/deep/head.fhtml",
        "include ../shared\n\nheader sticky\n  +logo()\n",
    ),
    (
        "partials/shared.fhtml",
        "#!shorthand\ndef brand()\n  strong fx \"fhtml\"\n\ndef logo()\n  span tc \"logo\"\n",
    ),
];

// ---- rendering ------------------------------------------------------------

#[test]
fn mem_and_disk_render_identically() {
    let (f, m) = both(LAYOUT);
    let src = LAYOUT[0].1;
    let data = Value::Null;
    let ctx = fhtml::json::parse("{\"who\": \"us\"}").unwrap();
    for mode in [Mode::Min, Mode::Pretty] {
        let disk = render_opts_from(
            src,
            Some(&f.root.join("main.fhtml")),
            &data,
            &ctx,
            &opts(mode),
        )
        .unwrap();
        let mem = render_opts_vfs(
            src,
            Some(Path::new("main.fhtml")),
            &data,
            &ctx,
            &opts(mode),
            &m,
        )
        .unwrap();
        assert_eq!(disk.html, mem.html);
        assert_eq!(disk.warnings, mem.warnings);
        // The shorthand actually decoded (its directive is scoped to
        // shared.fhtml) — guards against a silently-empty render.
        assert!(mem.html.contains("flex"), "got: {}", mem.html);
        assert!(mem.html.contains("text-center"));
    }
}

#[test]
fn mem_and_disk_compile_to_js_identically() {
    let (f, m) = both(LAYOUT);
    let src = LAYOUT[0].1;
    let disk =
        fhtml::compile_to_js_opts_from(src, Some(&f.root.join("main.fhtml")), &opts(Mode::Min))
            .unwrap();
    let mem =
        fhtml::compile_to_js_opts_vfs(src, Some(Path::new("main.fhtml")), &opts(Mode::Min), &m)
            .unwrap();
    assert_eq!(disk.html, mem.html);
    assert_eq!(disk.warnings, mem.warnings);
}

#[test]
fn deps_come_back_as_normalized_map_keys() {
    let (_f, m) = both(LAYOUT);
    let deps = deps_vfs(LAYOUT[0].1, Some(Path::new("main.fhtml")), &m).unwrap();
    // First-include order, includers before their own includes, and the
    // `..` spelling unified to the map key.
    assert_eq!(
        deps,
        vec![
            PathBuf::from("partials/lib.fhtml"),
            PathBuf::from("partials/deep/head.fhtml"),
            PathBuf::from("partials/shared.fhtml"),
        ]
    );
}

// ---- error parity ---------------------------------------------------------

#[test]
fn missing_include_errors_agree_on_position_and_shape() {
    let (f, m) = both(&[("main.fhtml", "include ./nope\n\np \"hi\"\n")]);
    let src = "include ./nope\n\np \"hi\"\n";
    let disk = render_opts_from(
        src,
        Some(&f.root.join("main.fhtml")),
        &Value::Null,
        &Value::Null,
        &opts(Mode::Min),
    )
    .unwrap_err();
    let mem = render_opts_vfs(
        src,
        Some(Path::new("main.fhtml")),
        &Value::Null,
        &Value::Null,
        &opts(Mode::Min),
        &m,
    )
    .unwrap_err();
    assert_eq!((disk.line, disk.col), (mem.line, mem.col));
    assert_eq!(
        mem.msg,
        "cannot include `nope.fhtml`: no such file in the file map"
    );
    let prefix = format!("cannot include `{}/nope.fhtml`: ", f.root.display());
    assert!(disk.msg.starts_with(&prefix), "got: {}", disk.msg);
}

#[test]
fn cycle_errors_agree_modulo_the_path_prefix() {
    let files: &[(&str, &str)] = &[
        ("main.fhtml", "include ./a\n"),
        ("a.fhtml", "include ./b\n"),
        ("b.fhtml", "include ./a\n"),
    ];
    let (f, m) = both(files);
    let disk = render_opts_from(
        files[0].1,
        Some(&f.root.join("main.fhtml")),
        &Value::Null,
        &Value::Null,
        &opts(Mode::Min),
    )
    .unwrap_err();
    let mem = render_opts_vfs(
        files[0].1,
        Some(Path::new("main.fhtml")),
        &Value::Null,
        &Value::Null,
        &opts(Mode::Min),
        &m,
    )
    .unwrap_err();
    assert_eq!((disk.line, disk.col), (mem.line, mem.col));
    assert!(mem.msg.contains("include cycle"), "got: {}", mem.msg);
    // Same text once the tempdir prefix is stripped from the disk version.
    let stripped = disk.msg.replace(&format!("{}/", f.root.display()), "");
    assert_eq!(stripped, mem.msg);
}

#[test]
fn def_collision_across_includes_matches() {
    let files: &[(&str, &str)] = &[
        ("main.fhtml", "def brand()\n  b \"x\"\n\ninclude ./lib\n"),
        ("lib.fhtml", "def brand()\n  i \"y\"\n"),
    ];
    let (f, m) = both(files);
    let disk = render_opts_from(
        files[0].1,
        Some(&f.root.join("main.fhtml")),
        &Value::Null,
        &Value::Null,
        &opts(Mode::Min),
    )
    .unwrap_err();
    let mem = render_opts_vfs(
        files[0].1,
        Some(Path::new("main.fhtml")),
        &Value::Null,
        &Value::Null,
        &opts(Mode::Min),
        &m,
    )
    .unwrap_err();
    assert_eq!(
        (disk.line, disk.col, &disk.msg),
        (mem.line, mem.col, &mem.msg)
    );
}

// ---- analysis -------------------------------------------------------------

#[test]
fn analyze_through_a_mem_vfs_matches_disk() {
    let files: &[(&str, &str)] = &[
        (
            "main.fhtml",
            "include ./partials/lib\n\ndiv\n  +badge(label=\"x\")\n",
        ),
        (
            "partials/lib.fhtml",
            "def badge(label)\n  span rounded \"{label}\"\n",
        ),
    ];
    let (f, m) = both(files);
    let disk = analyze(files[0].1, Some(&f.root.join("main.fhtml")));
    let mem = analyze_vfs(files[0].1, Some(Path::new("main.fhtml")), &m);

    assert!(disk.error.is_none() && mem.error.is_none());
    assert_eq!(
        mem.includes[0].resolved.as_deref(),
        Some(Path::new("partials/lib.fhtml"))
    );
    // Same symbols, same spans; only the file identity differs (canonical
    // disk path vs normalized map key).
    assert_eq!(disk.defs.len(), mem.defs.len());
    for (d, e) in disk.defs.iter().zip(&mem.defs) {
        assert_eq!(d.name, e.name);
        assert_eq!(d.name_span, e.name_span);
        assert_eq!(d.end_line, e.end_line);
        assert_eq!(d.file.is_some(), e.file.is_some());
    }
    let badge = mem.defs.iter().find(|d| d.name == "badge").unwrap();
    assert_eq!(badge.file.as_deref(), Some(Path::new("partials/lib.fhtml")));
    assert_eq!(disk.calls.len(), mem.calls.len());
    assert_eq!(mem.warnings.len(), disk.warnings.len());
}

#[test]
fn broken_buffer_rescan_chases_includes_through_the_map() {
    // Mid-keystroke entry (unclosed string) — included defs stay available,
    // exactly like the disk rescan (the LSP relies on this for completion).
    let mut m = MemVfs::new();
    m.add("lib.fhtml", "def badge(label tone=1)\n  span \"{label}\"\n");
    let src = "include ./lib\n\nspan \"unclosed\n";
    let a = analyze_vfs(src, Some(Path::new("main.fhtml")), &m);
    assert!(a.error.is_some());
    let badge = a.defs.iter().find(|d| d.name == "badge").expect("badge");
    assert_eq!(badge.file.as_deref(), Some(Path::new("lib.fhtml")));
    assert_eq!(
        a.includes[0].resolved.as_deref(),
        Some(Path::new("lib.fhtml"))
    );
}

#[test]
fn deps_from_still_reads_the_disk() {
    // The `_from` family must keep byte-identical disk behavior — spot-check
    // the delegation wiring.
    let f = Fixture::new();
    f.write("lib.fhtml", "def b()\n  i \"y\"\n");
    let main = f.write("main.fhtml", "include ./lib\n\n+b()\n");
    let src = fs::read_to_string(&main).unwrap();
    let deps = deps_from(&src, Some(&main)).unwrap();
    assert_eq!(
        deps,
        vec![fs::canonicalize(f.root.join("lib.fhtml")).unwrap()]
    );
}

// ---- the corpus gate ------------------------------------------------------

#[test]
fn corpus_renders_identically_through_a_mem_vfs() {
    let mut checked = 0;
    for dir in ["bench/out/fhtml", "site"] {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().and_then(|e| e.to_str()) != Some("fhtml") {
                continue;
            }
            let src = fs::read_to_string(&path).unwrap();
            let name = path.file_name().unwrap().to_str().unwrap();
            let mut m = MemVfs::new();
            m.add(name, src.as_str());
            for mode in [Mode::Min, Mode::Pretty] {
                let disk =
                    render_opts_from(&src, Some(&path), &Value::Null, &Value::Null, &opts(mode))
                        .unwrap();
                let mem = render_opts_vfs(
                    &src,
                    Some(Path::new(name)),
                    &Value::Null,
                    &Value::Null,
                    &opts(mode),
                    &m,
                )
                .unwrap();
                assert_eq!(disk.html, mem.html, "{}", path.display());
                assert_eq!(disk.warnings, mem.warnings, "{}", path.display());
            }
            checked += 1;
        }
    }
    assert!(checked >= 49, "expected the full corpus, checked {checked}");
}
