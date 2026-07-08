//! `--target=js` parity harness (SPEC §11 targets): the emitted ES module
//! must produce byte-identical output to the Rust renderer, for every
//! statement form, escaping edge, and number-stringification case. Needs
//! `node` on PATH; skips (with a note) when absent, like bench's pug check.

use std::fs;
use std::process::Command;

use fhtml::{compile_to_js, json, render_full, Mode, Value};

fn node_available() -> bool {
    Command::new("node")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success())
}

const RUNNER: &str = r#"
const [modPath, dataJson, ctxJson] = process.argv.slice(2);
const m = await import(modPath);
try {
    process.stdout.write(m.default(JSON.parse(dataJson), JSON.parse(ctxJson)));
} catch (e) {
    process.stdout.write("ERROR: " + e.message);
}
"#;

struct Harness {
    dir: std::path::PathBuf,
    counter: usize,
}

impl Harness {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("fhtml-jsgen-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("runner.mjs"), RUNNER).unwrap();
        Harness { dir, counter: 0 }
    }

    /// Renders `src` both ways and asserts byte equality (or identical
    /// error-ness when the Rust side errors).
    fn assert_parity(&mut self, src: &str, data_json: &str, ctx_json: &str, mode: Mode) {
        self.counter += 1;
        let module = compile_to_js(src, mode).unwrap().html;
        let path = self.dir.join(format!("m{}.mjs", self.counter));
        fs::write(&path, &module).unwrap();
        let out = Command::new("node")
            .arg(self.dir.join("runner.mjs"))
            .arg(&path)
            .arg(data_json)
            .arg(ctx_json)
            .output()
            .expect("node runs");
        assert!(
            out.status.success(),
            "node failed for {src:?}:\n{}\nmodule:\n{module}",
            String::from_utf8_lossy(&out.stderr)
        );
        let js_out = String::from_utf8(out.stdout).unwrap();

        let data: Value = json::parse(data_json).unwrap();
        let ctx: Value = json::parse(ctx_json).unwrap();
        match render_full(src, &data, &ctx, mode) {
            Ok(rust_out) => assert_eq!(
                js_out, rust_out.html,
                "JS/Rust divergence for {src:?} with {data_json}\nmodule:\n{module}"
            ),
            Err(e) => {
                let expected = format!("ERROR: {}:{}: error: {}", e.line, e.col, e.msg);
                assert_eq!(
                    js_out, expected,
                    "JS error divergence for {src:?} with {data_json}"
                );
            }
        }
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn js_target_matches_rust_renderer() {
    if !node_available() {
        eprintln!("skipping js parity test: `node` not found on PATH");
        return;
    }
    let mut h = Harness::new("parity");

    // Every statement form + nesting, both data shapes, both modes.
    let full = r#"doctype html
html(lang=en)
  body bg-white antialiased
    if user
      . #hero flex {user.admin ? 'ring-2 ring-red-500' : ''}
        h1 text-xl "Hi, {user.name}!"
        a(href={user.url} title="Profile of {user.name}") underline "profile"
    elif invited
      p "Almost in, {invited.by || 'someone'}"
    else
      p text-gray-400 "guest"
    ul divide-y
      for item, i in items
        li py-1 {i % 2 == 0 ? 'bg-gray-50' : ''}
          span "{i + 1}. {item.title}"
          if item.done
            span text-green-600 "done"
      empty
        li "nothing"
    for v, k in scores
      p "{k} = {v}"
    p
      | total: {n} item(s)
      | raw: {!snippet}
    // silent comment
    //! emitted comment
    li > a(href=/docs) font-medium "Docs"
    <div data-raw="kept &amp; verbatim">
      <span>raw</span>
    </div>
"#;
    let data_full = r#"{
        "user": {"name": "E & \"q\" <s>", "url": "/u/1?a=b&c=d", "admin": true},
        "items": [{"title": "x < y", "done": true}, {"title": "z"}],
        "scores": {"b": 2, "a": 1},
        "n": 2,
        "snippet": "<em>ok &amp; raw</em>"
    }"#;
    let data_alt = r#"{"invited": {"by": null}, "items": [], "n": 0}"#;
    for mode in [Mode::Pretty, Mode::Min] {
        h.assert_parity(full, data_full, r#"{"theme": "dark"}"#, mode);
        h.assert_parity(full, data_alt, "null", mode);
        h.assert_parity(full, "{}", "{}", mode);
    }

    // ctx reach + non-shadowing.
    h.assert_parity(
        "for item in items\n  p \"{item} on {ctx.theme}\"",
        r#"{"items": ["a", "b"], "ctx": "decoy"}"#,
        r#"{"theme": "dark"}"#,
        Mode::Min,
    );

    // Number stringification: integers, floats, negative zero, large/small
    // magnitudes, division results.
    h.assert_parity(
        r#"p "{3.0} {2.5} {-0} {1000000000000000000000 * 1} {x} {y} {1 / 3} {0.1 + 0.2} {-n} {big} {tiny}""#,
        r#"{"x": 1e21, "y": 1e-7, "n": 0.0000025, "big": 123456789012345680000000, "tiny": 2.5e-9}"#,
        "{}",
        Mode::Min,
    );

    // Expression semantics: precedence, equality (deep + order-insensitive
    // maps), + coercion, logic values, indexing, missing paths.
    h.assert_parity(
        r#"p "{1 + 2 * 3} {(1 + 2) * 3} {a == b} {m1 == m2} {'#' + 7} {7 + '#'} {null || 'dflt'} {'' && 'x'} {items[1]} {items[9]} {miss.deep} {!!items} {-2 + 1} {2 - -3} {'x' == 'x' ? m1.k : 'no'}""#,
        r#"{"a": [1, {"k": "v"}], "b": [1, {"k": "v"}], "m1": {"k": 1, "j": 2}, "m2": {"j": 2, "k": 1}, "items": [10, 20]}"#,
        "{}",
        Mode::Min,
    );

    // Escaping edges: quotes in attrs, raw with markup, class escaping,
    // literal braces, backslash text escapes.
    h.assert_parity(
        "p \"say \\\"hi\\\" \\{lit} {q}\"\na(title=\"He said \\\"{q}\\\"\")\ndiv {cls}\np\n  | {!html}",
        r#"{"q": "<&\">", "cls": "a<b\"c  d", "html": "<script>if (a && b < c) x();</script>"}"#,
        "{}",
        Mode::Min,
    );

    // Render errors carry identical position + message both sides.
    h.assert_parity(r#"p "{items + 1}""#, r#"{"items": []}"#, "{}", Mode::Min);
    h.assert_parity(
        "for c in word\n  p \"{c}\"",
        r#"{"word": "abc"}"#,
        "{}",
        Mode::Min,
    );
    h.assert_parity(r#"p "{1 / 0}""#, "{}", "{}", Mode::Min);
    h.assert_parity(r#"p "{n} items""#, r#"{"n": {"a": 1}}"#, "{}", Mode::Min);

    // Loop shadowing and unshadowing; nested loops over mixed shapes.
    h.assert_parity(
        "p \"{x}\"\nfor x, i in xs\n  for y in x.ys\n    p \"{i}:{y}\"\np \"{x}\"",
        r#"{"x": "outer", "xs": [{"ys": [1, 2]}, {"ys": []}]}"#,
        "{}",
        Mode::Min,
    );

    // Static file → constant function, byte-identical to compile().
    let static_src = "div flex\n  p text-sm \"plain\"\n";
    h.assert_parity(static_src, "{}", "{}", Mode::Pretty);
    h.assert_parity(static_src, "{}", "{}", Mode::Min);
}

#[test]
fn js_components_match_rust_renderer() {
    if !node_available() {
        eprintln!("skipping js parity test: `node` not found on PATH");
        return;
    }
    let mut h = Harness::new("components");

    // The SPEC §10.4 shapes in one file: params with expression defaults,
    // bare/quoted/unquoted args, `children`, a call nested under elements
    // and a loop (the def body renders at the call's depth — the dynamic
    // Pretty indent path), and a childless call to a children-using def.
    let full = r#"def card(title kind='note' n={1 + 1} wide=false)
  article rounded {wide ? 'w-full' : 'w-64'} {kind}
    h3 "{title} ({n})"
    . body
      children
section px-4
  for item, i in items
    +card(title="{i}: {item.name}" kind={item.kind || 'plain'} n={i * 10} wide={item.big})
      p "{item.desc}"
      if item.hot
        span "hot"
  +card(title="empty")
"#;
    let data = r#"{"items": [
        {"name": "A & B", "kind": "alert", "desc": "first <one>", "big": true, "hot": true},
        {"name": "c", "desc": "second"}
    ]}"#;
    for mode in [Mode::Pretty, Mode::Min] {
        h.assert_parity(full, data, "{}", mode);
        h.assert_parity(full, r#"{"items": []}"#, "{}", mode);
    }

    // Closed scopes (SPEC §10.3): inside the body, unbound names are null
    // and `ctx` still reaches; the same name resolves differently in the
    // caller's block.
    h.assert_parity(
        "def probe(x)\n  p \"{x}|{name}|{ctx.theme}\"\n  children\n+probe(x={name})\n  p \"{name}\"",
        r#"{"name": "root"}"#,
        r#"{"theme": "dark"}"#,
        Mode::Min,
    );

    // `children` through layers: a block passed to an inner call renders
    // the *outer* component's children, in the outer caller's scope —
    // and repeats when `children` appears twice.
    h.assert_parity(
        "def inner\n  . i\n    children\ndef outer\n  +inner\n    children\n    children\n+outer\n  p \"{name}\"",
        r#"{"name": "caller"}"#,
        "{}",
        Mode::Min,
    );

    // Recursion: a tree via data, exercising the depth counter and the
    // accumulated Pretty indent across recursive calls.
    let tree = "def tree(node)\n  li\n    span \"{node.label}\"\n    if node.kids\n      ul\n        for k in node.kids\n          +tree(node={k})\nul\n  +tree(node={root})\n";
    let tree_data = r#"{"root": {"label": "a", "kids": [
        {"label": "b", "kids": [{"label": "c"}]},
        {"label": "d"}
    ]}}"#;
    for mode in [Mode::Pretty, Mode::Min] {
        h.assert_parity(tree, tree_data, "{}", mode);
    }

    // Runtime errors carry identical position + message both sides: the
    // depth cap at the exceeding call site (mutual recursion), a bad
    // argument expression, and a bad default evaluated at the call.
    h.assert_parity("def a\n  +b\ndef b\n  +a\n+a\n", "{}", "{}", Mode::Min);
    h.assert_parity(
        "def c(n)\n  p \"{n}\"\n+c(n={items + 1})\n",
        r#"{"items": []}"#,
        "{}",
        Mode::Min,
    );
    h.assert_parity(
        "def c(n={miss / 2})\n  p \"{n}\"\n+c\n",
        r#"{"miss": "x"}"#,
        "{}",
        Mode::Min,
    );

    // The measured demo, both modes — the corpus golden as parity.
    let blog = include_str!("corpus/blog-cards-def.fhtml");
    for mode in [Mode::Pretty, Mode::Min] {
        h.assert_parity(blog, "null", "{}", mode);
    }
}

#[test]
fn js_component_checks_are_compile_errors() {
    // The static call checks (SPEC §10.4) run before code generation, so
    // for `--target=js` they are compile errors — same messages and
    // positions as the renderer, which reports them at render time. No
    // `node` needed here.
    let e = compile_to_js("+ghost\n", Mode::Min).unwrap_err();
    assert_eq!((e.line, e.col), (1, 1));
    assert!(
        e.msg.contains("unknown component `+ghost`"),
        "got: {}",
        e.msg
    );

    let src = "def c(a)\n  p \"{a}\"\nif false\n  +c(wrong=1)\n";
    let e = compile_to_js(src, Mode::Min).unwrap_err();
    assert!(e.msg.contains("unknown argument `wrong`"), "got: {}", e.msg);

    let e = compile_to_js("def c\n  p \"x\"\n+c\n  p \"dropped\"\n", Mode::Min).unwrap_err();
    assert!(e.msg.contains("never uses `children`"), "got: {}", e.msg);

    let e = compile_to_js("def c(a)\n  p \"{a}\"\n+c\n", Mode::Min).unwrap_err();
    assert!(e.msg.contains("missing argument `a`"), "got: {}", e.msg);
}

#[test]
fn js_includes_match_rust_renderer() {
    if !node_available() {
        eprintln!("skipping js include parity test: `node` not found on PATH");
        return;
    }
    use fhtml::{compile_to_js_from, render_full_from};
    let mut h = Harness::new("includes");

    // Fixture tree: a def library + a markup partial, both included.
    let files: &[(&str, &str)] = &[
        (
            "page.fhtml",
            "include ./partials/card\ninclude ./partials/banner\nmain mx-auto\n  for item in items\n    +card(title={item.t})\n      p \"{item.body}\"\n",
        ),
        (
            "partials/card.fhtml",
            "def card(title kind='note')\n  . rounded p-4 {kind}\n    h3 \"{title}\"\n    children\n",
        ),
        (
            "partials/banner.fhtml",
            "p text-xs \"rendered {ctx.site || 'somewhere'}\"\n",
        ),
        // Error parity: evaluation inside included content — the position is
        // remapped to the include site (SPEC §10.5) on BOTH backends.
        ("boom.fhtml", "p \"ok\"\ninclude ./partials/bad\n"),
        ("partials/bad.fhtml", "p \"{n / d}\"\n"),
    ];
    for (name, content) in files {
        let p = h.dir.join(name);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    let cases: &[(&str, &str, &str)] = &[
        (
            "page.fhtml",
            r#"{"items": [{"t": "A", "body": "first"}, {"t": "B", "body": "second"}]}"#,
            r#"{"site": "fhtml.dev"}"#,
        ),
        ("boom.fhtml", r#"{"n": 1, "d": 0}"#, "{}"),
    ];
    for (root, data_json, ctx_json) in cases {
        let path = h.dir.join(root);
        let src = fs::read_to_string(&path).unwrap();
        for mode in [Mode::Min, Mode::Pretty] {
            h.counter += 1;
            let module = compile_to_js_from(&src, Some(&path), mode).unwrap().html;
            let mod_path = h.dir.join(format!("inc{}.mjs", h.counter));
            fs::write(&mod_path, &module).unwrap();
            let out = Command::new("node")
                .arg(h.dir.join("runner.mjs"))
                .arg(&mod_path)
                .arg(data_json)
                .arg(ctx_json)
                .output()
                .expect("node runs");
            assert!(out.status.success(), "node failed for {root}");
            let js_out = String::from_utf8(out.stdout).unwrap();
            let data = json::parse(data_json).unwrap();
            let ctx = json::parse(ctx_json).unwrap();
            match render_full_from(&src, Some(&path), &data, &ctx, mode) {
                Ok(rust_out) => assert_eq!(js_out, rust_out.html, "{root} mode {mode:?}"),
                Err(e) => {
                    let expected = format!("ERROR: {}:{}: error: {}", e.line, e.col, e.msg);
                    assert_eq!(js_out, expected, "{root} error parity, mode {mode:?}");
                    assert_eq!((e.line, e.col), (2, 1), "remap to the include site");
                }
            }
        }
    }
}
