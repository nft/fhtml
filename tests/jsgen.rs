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
