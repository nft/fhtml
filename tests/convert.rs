//! html2fhtml converter tests.
//! Run with `cargo test --features convert`.
#![cfg(feature = "convert")]

use fhtml::convert::{check, compare_html, convert, Options};
use fhtml::{compile, format, Mode};

fn conv(html: &str) -> String {
    convert(html, &Options::default()).fhtml
}

fn assert_roundtrip(html: &str) {
    if let Err(e) = check(html, &Options::default()) {
        panic!("round-trip mismatch for {html:?}:\n{e}");
    }
}

// ── Goldens (plan §6.2): idiomatic output, not merely valid ─────────────────

/// The sample card: its compiled HTML converts back to exactly the
/// canonical fhtml (with `.` for the divs).
#[test]
fn golden_project_card() {
    let card = r#". flex items-center gap-4 rounded-xl bg-white p-6 shadow-md
  img(src=/img/ava.jpg alt="Erin's avatar") size-12 rounded-full
  .
    p text-lg font-semibold text-gray-900 "Erin Lindford"
    p text-gray-500 "Product Engineer"
  button ml-auto rounded-full px-4 py-1 text-sm hover:bg-purple-600 hover:text-white "Message"
"#;
    let html = compile(card, Mode::Pretty).unwrap();
    assert_eq!(conv(&html), card);
    assert_roundtrip(&html);
}

/// The SPEC torture button: every Tailwind class survives byte-for-byte
/// (converter output is one logical line — no `\` wrapping).
#[test]
fn golden_torture_button() {
    let src = "button inline-flex gap-2 rounded-sm border px-4 py-2.5 text-sm font-semibold cursor-pointer \\\n       text-center align-middle text-zinc-900 bg-zinc-100 border-zinc-200 \\\n       transition-all duration-200 \\\n       hover:bg-zinc-200 hover:border-zinc-300 \\\n       active:translate-y-[0.5px] active:bg-zinc-200 active:border-zinc-300 active:shadow-none \\\n       focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-zinc-900 \\\n       dark:bg-neutral-700 dark:text-zinc-300 dark:border-zinc-700 \\\n       dark:hover:bg-zinc-950 dark:hover:border-zinc-950 \\\n       dark:active:bg-zinc-900 dark:active:border-zinc-950 \\\n       dark:focus-visible:outline-zinc-200 \\\n       \"Save\"\n";
    let html = compile(src, Mode::Pretty).unwrap();
    let out = conv(&html);
    assert!(out.starts_with("button inline-flex gap-2"));
    assert!(out.contains("active:translate-y-[0.5px]"));
    assert!(out.contains("dark:focus-visible:outline-zinc-200 \"Save\"\n"));
    assert_eq!(
        compile(&out, Mode::Min).unwrap(),
        compile(src, Mode::Min).unwrap()
    );
    assert_roundtrip(&html);
}

// ── Structure ───────────────────────────────────────────────────────────────

#[test]
fn boilerplate_unwrapped() {
    assert_eq!(conv("<p>hi</p>"), "p \"hi\"\n");
}

#[test]
fn full_document_preserved() {
    let out = conv("<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><title>T</title></head><body><h1>Hi</h1></body></html>");
    assert_eq!(
        out,
        "doctype\nhtml(lang=en)\n  head\n    meta(charset=utf-8)\n    title \"T\"\n  body > h1 \"Hi\"\n"
    );
    assert_roundtrip("<!DOCTYPE html><html lang=\"en\"><head><title>T</title></head><body><h1>Hi</h1></body></html>");
}

#[test]
fn head_content_blocks_unwrap() {
    // No doctype, but a <title> means the scaffolding carries content.
    let out = conv("<title>T</title><p>x</p>");
    assert!(out.starts_with("html\n"), "{out}");
    assert!(out.contains("title \"T\""));
}

#[test]
fn chain_synthesis() {
    assert_eq!(
        conv("<nav><ul><li>one</li></ul></nav>"),
        "nav > ul > li \"one\"\n"
    );
}

#[test]
fn no_chains_flag() {
    let opts = Options {
        chains: false,
        ..Options::default()
    };
    assert_eq!(
        convert("<nav><ul><li>one</li></ul></nav>", &opts).fhtml,
        "nav\n  ul\n    li \"one\"\n"
    );
}

/// A comment sibling blocks chaining — chain children attach to the
/// innermost element, which would move the comment into it (plan §3).
#[test]
fn comment_blocks_chain() {
    let out = conv("<div><!-- why --><a href=\"/x\">go</a></div>");
    assert_eq!(out, ".\n  //! why\n  a(href=/x) \"go\"\n");
    assert_roundtrip("<div><!-- why --><a href=\"/x\">go</a></div>");
}

#[test]
fn fragment_context_rescues_table_rows() {
    // Document parsing foster-parents a bare <tr> away entirely (only the
    // text survives) — the `--fragment=table` context keeps the row. The
    // tbody is the HTML parser's own spec-mandated synthesis.
    assert_eq!(conv("<tr><td>x</td></tr>"), "| x\n");
    let opts = Options {
        fragment: Some("table".to_string()),
        ..Options::default()
    };
    assert_eq!(
        convert("<tr><td>x</td></tr>", &opts).fhtml,
        "tbody > tr > td \"x\"\n"
    );
    check("<tr><td>x</td></tr>", &opts).unwrap();
}

// ── Attributes ──────────────────────────────────────────────────────────────

#[test]
fn boolean_attrs_go_bare() {
    let out = conv("<input type=\"checkbox\" checked disabled=\"disabled\" required=\"\">");
    assert_eq!(out, "input(type=checkbox checked disabled required)\n");
    assert_roundtrip("<input type=\"checkbox\" checked disabled=\"disabled\" required=\"\">");
}

#[test]
fn empty_non_boolean_keeps_value() {
    // alt="" is meaningful (decorative image) — empty ≠ boolean here.
    assert_eq!(
        conv("<img src=\"x.png\" alt=\"\">"),
        "img(src=x.png alt=\"\")\n"
    );
}

#[test]
fn id_becomes_token() {
    assert_eq!(conv("<div id=\"hero\">x</div>"), ". #hero \"x\"\n");
}

#[test]
fn hostile_id_stays_attr() {
    let out = conv("<div id=\"a b\">x</div>");
    assert_eq!(out, ".(id=\"a b\") \"x\"\n");
    assert_roundtrip("<div id=\"a b\">x</div>");
}

#[test]
fn hostile_classes_ride_in_class_attr() {
    let out = conv("<div class=\"a #b {c\">x</div>");
    assert_eq!(out, ".(class=\"a #b \\{c\") \"x\"\n");
    assert_roundtrip("<div class=\"a #b {c\">x</div>");
}

#[test]
fn angular_attr_names_fall_back_to_raw() {
    let res = convert("<div (click)=\"do()\">y</div>", &Options::default());
    assert!(
        res.fhtml.starts_with("<div (click)=\"do()\">"),
        "{}",
        res.fhtml
    );
    assert!(!res.warnings.is_empty());
    assert_roundtrip("<div (click)=\"do()\">y</div>");
}

#[test]
fn newline_attr_normalized_with_warning() {
    let res = convert(
        "<div title=\"line one\nline two\">x</div>",
        &Options::default(),
    );
    assert_eq!(res.fhtml, ".(title=\"line one line two\") \"x\"\n");
    assert_eq!(res.warnings.len(), 1);
}

/// Collapsing a newline before a `//` comment would comment out the rest of
/// the value — raw tag lines preserve it instead (plan §3).
#[test]
fn multiline_js_attr_falls_back_to_raw() {
    let html = "<div x-data=\"{\n  open: false, // toggle\n  n: 1\n}\"><p>c</p></div>";
    let res = convert(html, &Options::default());
    assert!(res.fhtml.starts_with("<div x-data=\"{\n"), "{}", res.fhtml);
    assert!(res.fhtml.contains("// toggle"));
    assert!(res.fhtml.contains("p \"c\""));
    assert!(res.fhtml.trim_end().ends_with("</div>"));
    assert!(!res.warnings.is_empty());
    assert_roundtrip(html);
}

// ── Text and whitespace (plan §4) ───────────────────────────────────────────

#[test]
fn mixed_inline_text_keeps_rendered_spaces() {
    let html = "<li><a href=\"/d\">See <b>docs</b> now</a></li>";
    assert_eq!(
        conv(html),
        "li > a(href=/d)\n  | See\n  |\n  b \"docs\"\n  |  now\n"
    );
    assert_roundtrip(html);
}

#[test]
fn interelement_whitespace_dropped() {
    assert_eq!(
        conv("<div>\n  <p>a</p>\n  <p>b</p>\n</div>"),
        ".\n  p \"a\"\n  p \"b\"\n"
    );
}

#[test]
fn long_text_prefers_pipe_line() {
    let html = "<p>This sentence is deliberately longer than eighty characters so that the converter prefers a pipe line.</p>";
    let out = conv(html);
    assert!(out.starts_with("p\n  | This sentence"), "{out}");
    assert_roundtrip(html);
}

#[test]
fn text_with_quotes_prefers_pipe_line() {
    let html = "<p>He said \"hi\" to me</p>";
    assert_eq!(conv(html), "p\n  | He said \"hi\" to me\n");
    assert_roundtrip(html);
}

#[test]
fn entities_roundtrip() {
    let html = "<p>a &amp; b &lt;tag&gt; &nbsp;end</p>";
    // NBSP is not HTML whitespace: it survives collapsing.
    assert_eq!(conv(html), "p \"a & b <tag> \u{a0}end\"\n");
    assert_roundtrip(html);
}

// ── Raw passthrough (plan §3) ───────────────────────────────────────────────

#[test]
fn pre_is_single_line_entity_encoded() {
    let html = "<pre>a\n\tb\n   c</pre>";
    assert_eq!(conv(html), "<pre>a&#10;&#9;b&#10;   c</pre>\n");
    assert_roundtrip(html);
}

/// The parser drops one newline right after `<pre>` — the converter re-adds
/// it so a leading blank line survives the round trip.
#[test]
fn pre_leading_newline_survives() {
    assert_roundtrip("<pre>\n\ntwo blank-ish lines</pre>");
    assert_roundtrip("<textarea>\n  indented</textarea>");
}

#[test]
fn script_stays_multiline() {
    // (Wrapped in a div — a bare <script> parses into <head>.)
    let html = "<div><script>\nif (a < b) {\n  go()\n}\n</script></div>";
    let out = conv(html);
    assert!(out.contains("<script>\n"), "{out}");
    assert!(out.contains("if (a < b)"));
    assert_roundtrip(html);
}

#[test]
fn script_backtick_warns() {
    let res = convert("<script>let s = `a\nb`;</script>", &Options::default());
    assert!(
        res.warnings.iter().any(|w| w.contains("backtick")),
        "{:?}",
        res.warnings
    );
}

#[test]
fn svg_raw_by_default() {
    let html = "<svg viewBox=\"0 0 24 24\"><path d=\"M4 6h16\"/></svg>";
    let out = conv(html);
    assert!(out.starts_with("<svg"), "{out}");
    assert!(out.contains("viewBox=\"0 0 24 24\""));
    assert_roundtrip(html);
}

#[test]
fn svg_converted_on_request() {
    let opts = Options {
        convert_svg: true,
        ..Options::default()
    };
    let html =
        "<svg viewBox=\"0 0 24 24\" fill=\"none\"><path d=\"M4 6h16\" stroke-width=\"2\"/></svg>";
    assert_eq!(
        convert(html, &opts).fhtml,
        "svg(viewBox=\"0 0 24 24\" fill=none) > path(d=\"M4 6h16\" stroke-width=2)\n"
    );
    check(html, &opts).unwrap();
}

/// `<slot>` is an ordinary element since the template statement was renamed
/// to `children` (SPEC §13) — no raw fallback, no warning.
#[test]
fn slot_is_an_ordinary_element() {
    let html = "<slot name=\"icon\"><span class=\"text-xs\">fallback</span></slot>";
    let res = convert(html, &Options::default());
    assert_eq!(res.fhtml, "slot(name=icon) > span text-xs \"fallback\"\n");
    assert!(res.warnings.is_empty(), "{:?}", res.warnings);
    assert_roundtrip(html);
}

/// Reserved-word tags: raw open/close tag lines, fhtml children between —
/// only the tag lines are raw (plan §3).
#[test]
fn reserved_tag_falls_back_to_raw_lines() {
    let html = "<div><if condition=\"x\"><span class=\"text-xs\">fallback</span></if></div>";
    let res = convert(html, &Options::default());
    assert!(
        res.fhtml
            .contains("<if condition=\"x\">\n  span text-xs \"fallback\"\n  </if>"),
        "{}",
        res.fhtml
    );
    assert!(!res.warnings.is_empty());
    assert_roundtrip(html);
}

#[test]
fn template_children_convert() {
    // (Wrapped in a div — a bare <template> parses into <head>.)
    let html = "<div><template id=\"row\"><tr><td>x</td></tr></template></div>";
    let out = conv(html);
    assert!(out.contains("template #row"), "{out}");
    assert!(out.contains("td \"x\""));
    assert_roundtrip(html);
}

#[test]
fn comments_emit_visible() {
    assert_eq!(
        conv("<div><!-- a note --><p>x</p></div>"),
        ".\n  //! a note\n  p \"x\"\n"
    );
}

// ── Determinism and canonicality (plan §6.5) ────────────────────────────────

#[test]
fn deterministic_and_canonical() {
    let html = "<div class=\"a\"><a href=\"/d\" id=\"x\">See <b>docs</b> now</a><ul><li><a href=\"/a\">A</a></li></ul><pre>x\ny</pre></div>";
    let once = conv(html);
    assert_eq!(once, conv(html), "conversion must be deterministic");
    assert_eq!(
        format(&once).unwrap(),
        once,
        "converter output must already be canonical"
    );
}

// ── Reverse goldens (plan §6.3): compiled fhtml survives the loop ───────────

#[test]
fn reverse_goldens() {
    let sources = [
        "doctype\nhtml(lang=en)\n  head\n    meta(charset=utf-8)\n    title \"Docs\"\n  body\n    h1 text-2xl \"Hello\"\n",
        ". #hero flex gap-2\n  a(href=/a target=_blank) underline \"Go\"\n  input(type=text required)\n",
        "p\n  | line one\n  | line two\n",
        "ul > li > a(href=/x) \"deep\"\n",
        "//! shipping note\n. p-4 \"done\"\n",
    ];
    for src in sources {
        for mode in [Mode::Pretty, Mode::Min] {
            let html = compile(src, mode).unwrap();
            if let Err(e) = check(&html, &Options::default()) {
                panic!("reverse golden failed ({mode:?}) for:\n{src}\n{e}");
            }
        }
    }
}

// ── DOM equivalence (benchmark grader) ──────────────────────────────────────

#[test]
fn compare_html_normalizes_like_check() {
    let opts = Options::default();
    // Formatting, comments, attr order, and boolean attr forms don't count.
    compare_html(
        "<div class=\"a b\"   id=x><!-- c --><input disabled></div>",
        "<div id=\"x\" class=\"a  b\">\n  <input disabled=\"disabled\">\n</div>",
        &opts,
    )
    .unwrap();
    // Real differences do.
    let err = compare_html("<p>hi</p>", "<p>ho</p>", &opts).unwrap_err();
    assert!(err.contains("text"), "unexpected diff message: {err}");
    compare_html("<p class=\"a\">x</p>", "<p class=\"b\">x</p>", &opts).unwrap_err();
}

// ── Corpus (plan §6.4) ──────────────────────────────────────────────────────

#[test]
fn corpus_roundtrips() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/corpus");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).expect("tests/corpus must exist") {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "html") {
            let html = std::fs::read_to_string(&path).unwrap();
            if let Err(e) = check(&html, &Options::default()) {
                panic!("corpus round-trip failed for {}:\n{e}", path.display());
            }
            count += 1;
        }
    }
    assert!(count >= 2, "corpus should contain samples, found {count}");
}

// ── Rendered output: pretty/min element-tree invariant (SPEC §11) ───────────

/// The §11 same-element-tree contract extends to template rendering: the
/// pretty and min renders of a template file must be normalized-DOM equal.
#[test]
fn rendered_pretty_min_same_element_tree() {
    let src = r#"doctype html
html(lang=en)
  body bg-white
    if user
      . #greet flex {user.admin ? 'ring-2' : ''}
        p text-lg "Hi, {user.name} & co. <3"
        a(href={user.url} title="Profile of {user.name}") "profile"
    else
      p "guest"
    ul
      for item, i in items
        li py-1 "{i + 1}. {item}"
      empty
        li "none"
    p
      | total: {n}
      | raw: {!snippet}
"#;
    let data = fhtml::json::parse(
        r#"{
        "user": {"name": "E & \"quotes\"", "url": "/u/1?a=b&c=d", "admin": true},
        "items": ["x < y", "z"],
        "n": 3,
        "snippet": "<em>ok</em>"
    }"#,
    )
    .unwrap();
    let pretty = fhtml::render(src, &data, Mode::Pretty).unwrap();
    let min = fhtml::render(src, &data, Mode::Min).unwrap();
    if let Err(e) = compare_html(&pretty, &min, &Options::default()) {
        panic!("pretty/min DOM mismatch:\n{e}\npretty:\n{pretty}\nmin:\n{min}");
    }
}
