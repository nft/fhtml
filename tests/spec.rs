//! Spec conformance tests. Golden inputs come verbatim from the spec and
//! SPEC.md — any grammar change that breaks them is rejected (SPEC §4.2).

use fhtml::{compile, Mode};

fn pretty(src: &str) -> String {
    compile(src, Mode::Pretty).unwrap()
}

fn min(src: &str) -> String {
    compile(src, Mode::Min).unwrap()
}

fn error(src: &str) -> String {
    compile(src, Mode::Min).unwrap_err().to_string()
}

// ---------------------------------------------------------------- goldens

#[test]
fn golden_card_from_project_md() {
    let src = r#"div flex items-center gap-4 rounded-xl bg-white p-6 shadow-md
  img(src=/img/ava.jpg alt="Erin's avatar") size-12 rounded-full
  .
    p text-lg font-semibold text-gray-900 "Erin Lindford"
    p text-gray-500 "Product Engineer"
  button ml-auto rounded-full px-4 py-1 text-sm hover:bg-purple-600 hover:text-white "Message"
"#;
    let expected = r#"<div class="flex items-center gap-4 rounded-xl bg-white p-6 shadow-md">
  <img src="/img/ava.jpg" alt="Erin's avatar" class="size-12 rounded-full">
  <div>
    <p class="text-lg font-semibold text-gray-900">Erin Lindford</p>
    <p class="text-gray-500">Product Engineer</p>
  </div>
  <button class="ml-auto rounded-full px-4 py-1 text-sm hover:bg-purple-600 hover:text-white">Message</button>
</div>
"#;
    assert_eq!(pretty(src), expected);
}

#[test]
fn golden_torture_button_from_spec() {
    let src = r#"button inline-flex gap-2 rounded-sm border px-4 py-2.5 text-sm font-semibold cursor-pointer \
       text-center align-middle text-zinc-900 bg-zinc-100 border-zinc-200 \
       transition-all duration-200 \
       hover:bg-zinc-200 hover:border-zinc-300 \
       active:translate-y-[0.5px] active:bg-zinc-200 active:border-zinc-300 active:shadow-none \
       focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-zinc-900 \
       dark:bg-neutral-700 dark:text-zinc-300 dark:border-zinc-700 \
       dark:hover:bg-zinc-950 dark:hover:border-zinc-950 \
       dark:active:bg-zinc-900 dark:active:border-zinc-950 \
       dark:focus-visible:outline-zinc-200 \
       "Save"
"#;
    let expected = "<button class=\"inline-flex gap-2 rounded-sm border px-4 py-2.5 text-sm \
font-semibold cursor-pointer text-center align-middle text-zinc-900 bg-zinc-100 \
border-zinc-200 transition-all duration-200 hover:bg-zinc-200 hover:border-zinc-300 \
active:translate-y-[0.5px] active:bg-zinc-200 active:border-zinc-300 active:shadow-none \
focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-zinc-900 \
dark:bg-neutral-700 dark:text-zinc-300 dark:border-zinc-700 dark:hover:bg-zinc-950 \
dark:hover:border-zinc-950 dark:active:bg-zinc-900 dark:active:border-zinc-950 \
dark:focus-visible:outline-zinc-200\">Save</button>";
    assert_eq!(min(src), expected);
}

// ------------------------------------------------- tokenizer contract §4.2

#[test]
fn tailwind_hazard_classes_pass_verbatim() {
    // Every row of the SPEC §4.2 hazard table.
    let src = ". dark:focus-visible:outline-zinc-200 py-2.5 active:translate-y-[0.5px] \
data-[state=open]:bg-red-500 bg-[url(/img/x.png)] [&>li]:mt-0 w-1/2 bg-black/50 text-sm/6 \
!mt-0 -mt-2 *:pt-2 @lg:flex";
    let expected = "<div class=\"dark:focus-visible:outline-zinc-200 py-2.5 \
active:translate-y-[0.5px] data-[state=open]:bg-red-500 bg-[url(/img/x.png)] [&>li]:mt-0 \
w-1/2 bg-black/50 text-sm/6 !mt-0 -mt-2 *:pt-2 @lg:flex\"></div>";
    assert_eq!(min(src), expected);
}

#[test]
fn parens_inside_quoted_attr_values() {
    // SPEC §4.3: the attrs segment ends at the first `)` outside quotes.
    let src = r#"button(onclick="alert('(hi)')") "Hi""#;
    assert_eq!(min(src), r#"<button onclick="alert('(hi)')">Hi</button>"#);
}

// ------------------------------------------------------------- element line

#[test]
fn id_attr_class_order_is_deterministic() {
    // SPEC §11: id, then paren attrs in source order, then merged class.
    let src = r#"a(href=/x target=_blank) #main flex "Go""#;
    assert_eq!(
        min(src),
        r#"<a id="main" href="/x" target="_blank" class="flex">Go</a>"#
    );
}

#[test]
fn class_attr_merges_before_bare_classes() {
    let src = r#"div(class="a b") c d"#;
    assert_eq!(min(src), r#"<div class="a b c d"></div>"#);
}

#[test]
fn boolean_attributes() {
    let src = "input(type=checkbox disabled required)";
    assert_eq!(min(src), r#"<input type="checkbox" disabled required>"#);
}

#[test]
fn dot_is_div_and_takes_attrs() {
    assert_eq!(min(". flex"), r#"<div class="flex"></div>"#);
    assert_eq!(
        min(".(role=main) flex"),
        r#"<div role="main" class="flex"></div>"#
    );
}

#[test]
fn chain_inline_when_leaf() {
    let src = "li > a(href=/docs) font-medium hover:underline \"Docs\"";
    assert_eq!(
        min(src),
        r#"<li><a href="/docs" class="font-medium hover:underline">Docs</a></li>"#
    );
}

#[test]
fn chain_children_attach_to_innermost() {
    let src = "li > a(href=/docs) \"Docs\"\n  span text-xs \"(new)\"\n";
    let expected =
        "<li><a href=\"/docs\">Docs\n  <span class=\"text-xs\">(new)</span>\n</a></li>\n";
    assert_eq!(pretty(src), expected);
}

#[test]
fn chain_onto_void_element() {
    let src = "label > input(type=text)";
    assert_eq!(min(src), r#"<label><input type="text"></label>"#);
}

#[test]
fn text_before_chain_belongs_to_outer() {
    let src = r#"li "See " > a(href=/x) "docs""#;
    assert_eq!(min(src), r#"<li>See <a href="/x">docs</a></li>"#);
}

// ------------------------------------------------------------ text & escaping

#[test]
fn text_is_entity_escaped() {
    assert_eq!(min(r#"p "a < b & c""#), "<p>a &lt; b &amp; c</p>");
}

#[test]
fn text_escapes_decode() {
    assert_eq!(min(r#"p "say \"hi\" \{ok}""#), "<p>say \"hi\" {ok}</p>");
}

#[test]
fn text_blocks_preserve_quotes_and_lines() {
    let src = "p text-sm\n  | He said \"hello\" and left.\n  | Second line.\n";
    let expected = "<p class=\"text-sm\">\n  He said \"hello\" and left.\n  Second line.\n</p>\n";
    assert_eq!(pretty(src), expected);
}

#[test]
fn text_block_lines_stay_separate_in_min() {
    let src = "p\n  | one\n  | two\n";
    assert_eq!(min(src), "<p>one\ntwo</p>");
}

// ----------------------------------------------------------- raw & comments

#[test]
fn raw_passthrough_with_dedent() {
    let src = "div p-4\n  <svg viewBox=\"0 0 24 24\">\n    <path d=\"M0 0\"/>\n  </svg>\n";
    let expected = "<div class=\"p-4\">\n  <svg viewBox=\"0 0 24 24\">\n    <path d=\"M0 0\"/>\n  </svg>\n</div>\n";
    assert_eq!(pretty(src), expected);
}

#[test]
fn comments_silent_and_emitted() {
    let src = "// dropped\n//! kept\np \"x\"\n";
    assert_eq!(pretty(src), "<!-- kept -->\n<p>x</p>\n");
}

// ------------------------------------------------- raw-text elements §6.3

mod raw_text {
    use super::{error, min, pretty};
    use fhtml::{compile_opts, Mode, Options};

    #[test]
    fn script_body_is_verbatim_not_escaped() {
        let src = "script\n  | if (a && b < c) go();\n";
        assert_eq!(min(src), "<script>if (a && b < c) go();</script>");
    }

    #[test]
    fn style_body_is_verbatim() {
        let src = "style\n  | .a > .b { color: red; }\n";
        assert_eq!(min(src), "<style>.a > .b { color: red; }</style>");
    }

    #[test]
    fn braces_are_literal_and_backslash_brace_stays_two_chars() {
        // No interpolation, no source escapes: `{` is a byte, `\{` is two.
        let src = "script\n  | let re = /\\{/; f({a: 1});\n";
        assert_eq!(min(src), "<script>let re = /\\{/; f({a: 1});</script>");
    }

    #[test]
    fn interpolation_stays_inert_under_render() {
        let src = "script\n  | send({user.name});\n";
        let data = fhtml::json::parse(r#"{"user": {"name": "Erin"}}"#).unwrap();
        assert_eq!(
            fhtml::render(src, &data, Mode::Min).unwrap(),
            "<script>send({user.name});</script>"
        );
    }

    #[test]
    fn body_bytes_identical_min_and_pretty() {
        let src = "div\n  script\n    | let x = 1;\n    |   x += 2;\n";
        let body = "<script>let x = 1;\n  x += 2;</script>";
        assert!(pretty(src).contains(body), "pretty: {}", pretty(src));
        assert!(min(src).contains(body), "min: {}", min(src));
    }

    #[test]
    fn leading_space_after_pipe_stripped_like_6_2() {
        // `|` + two spaces keeps one; a bare `|` is an empty line.
        let src = "script\n  |  indented();\n  |\n  | done();\n";
        assert_eq!(min(src), "<script> indented();\n\ndone();</script>");
    }

    #[test]
    fn tag_emitted_as_authored_but_matched_case_insensitively() {
        let src = "SCRIPT\n  | a && b\n";
        assert_eq!(min(src), "<SCRIPT>a && b</SCRIPT>");
    }

    #[test]
    fn empty_body_and_src_attr_untouched() {
        assert_eq!(min("script(src=/a.js)"), r#"<script src="/a.js"></script>"#);
        assert_eq!(
            pretty("script(src=/a.js)"),
            "<script src=\"/a.js\"></script>\n"
        );
    }

    #[test]
    fn no_templates_mode_is_identical() {
        let src = "script\n  | if (x) { f({a: 1}) }\n";
        let out = compile_opts(
            src,
            &Options {
                templates: false,
                ..Options::default()
            },
        )
        .unwrap();
        assert_eq!(out.html, "<script>if (x) { f({a: 1}) }</script>");
    }

    #[test]
    fn inline_text_on_tag_line_errors() {
        let e = error("script \"alert(1)\"");
        assert!(e.contains("raw text"), "got: {e}");
        assert!(e.contains("1:8"), "got: {e}");
    }

    #[test]
    fn chain_from_raw_text_element_errors() {
        let e = error("script > div \"x\"");
        assert!(e.contains("cannot chain"), "got: {e}");
    }

    #[test]
    fn element_child_errors() {
        let e = error("script\n  div \"x\"\n");
        assert!(e.contains("raw text"), "got: {e}");
        assert!(e.contains("2:1"), "got: {e}");
    }

    #[test]
    fn statement_child_errors() {
        let e = error("script\n  if x\n    | y\n");
        assert!(e.contains("`if` cannot nest inside `script`"), "got: {e}");
    }

    #[test]
    fn call_child_errors() {
        let e = error("style\n  +card\n");
        assert!(e.contains("`+card` cannot nest inside `style`"), "got: {e}");
    }

    #[test]
    fn end_tag_in_body_errors_with_position() {
        let e = error("script\n  | x('</script>');\n");
        assert!(e.contains("would end the `script` element"), "got: {e}");
        // Columns count within the line's content, indent excluded — the
        // `<` of `</script>` in `| x('</script>');` sits at column 6.
        assert!(e.contains("2:6"), "got: {e}");
    }

    #[test]
    fn end_tag_match_is_case_insensitive() {
        let e = error("script\n  | x('</SCRIPT foo');\n");
        assert!(e.contains("would end the `script` element"), "got: {e}");
    }

    #[test]
    fn end_tag_at_end_of_line_errors() {
        // Lines join with `\n`, which HTML reads as tag-name-terminating
        // whitespace — end-of-line counts.
        let e = error("script\n  | a = '</script\n");
        assert!(e.contains("would end the `script` element"), "got: {e}");
    }

    #[test]
    fn longer_tag_names_are_legal_text() {
        // `</scripting>` is not an end-tag per the script-data states.
        let src = "script\n  | x = '</scripting>' + '</style>';\n";
        assert_eq!(
            min(src),
            "<script>x = '</scripting>' + '</style>';</script>"
        );
    }

    #[test]
    fn chained_target_script_takes_raw_body() {
        let src = "div p-4 > script\n  | go();\n";
        assert_eq!(min(src), "<div class=\"p-4\"><script>go();</script></div>");
    }
}

// -------------------------------------------------------------- document

#[test]
fn doctype_and_page_skeleton() {
    let src = "doctype html\nhtml(lang=en)\n  head\n    title \"Hi\"\n  body bg-white\n";
    let expected = "<!DOCTYPE html>\n<html lang=\"en\">\n  <head>\n    <title>Hi</title>\n  </head>\n  <body class=\"bg-white\"></body>\n</html>\n";
    assert_eq!(pretty(src), expected);
}

#[test]
fn doctype_bare_also_works() {
    assert_eq!(min("doctype"), "<!DOCTYPE html>");
}

// ---------------------------------------------------------------- errors

#[test]
fn error_pug_class_shorthand() {
    let e = error(".card p-4");
    assert!(e.contains("Pug syntax"), "got: {e}");
    assert!(e.contains(". card"), "got: {e}");
}

#[test]
fn error_pug_id_start() {
    let e = error("#hero flex");
    assert!(e.contains(". #hero"), "got: {e}");
}

#[test]
fn error_pug_tag_suffix() {
    let e = error("div.card");
    assert!(e.contains("Pug syntax"), "got: {e}");
}

#[test]
fn error_template_constructs_are_not_static() {
    // SPEC §11: `compile` is the static path; template files need `render`.
    let e = error("if user\n  p \"hi\"");
    assert!(e.contains("static"), "got: {e}");
    let e = error("div {active}");
    assert!(e.contains("static"), "got: {e}");
}

#[test]
fn error_components_are_not_static() {
    // SPEC §11: `def`/`+call` are template constructs — the static path
    // rejects them like any other (they parse; render is the other path).
    let e = error("+card(title=\"x\")");
    assert!(e.contains("static"), "got: {e}");
    let e = error("def card(title)\n  p \"x\"");
    assert!(e.contains("static"), "got: {e}");
}

#[test]
fn error_include_is_not_static() {
    // SPEC §11: `include` is a template construct on the static path, like
    // `def` — resolution lives on the render path (mod includes, §10.5).
    let e = error("include ./partials/head");
    assert!(e.contains("`include`"), "got: {e}");
    assert!(e.contains("static"), "got: {e}");
}

#[test]
fn error_void_with_children() {
    let e = error("img(src=/x.png)\n  p \"nope\"");
    assert!(e.contains("void element"), "got: {e}");
}

#[test]
fn error_duplicate_id() {
    let e = error("div #a #b");
    assert!(e.contains("at most one id"), "got: {e}");
}

#[test]
fn error_duplicate_attr() {
    let e = error("a(href=/x href=/y)");
    assert!(e.contains("duplicate attribute"), "got: {e}");
}

#[test]
fn error_class_after_text() {
    let e = error("p \"text\" flex");
    assert!(e.contains("chain may follow inline text"), "got: {e}");
}

#[test]
fn error_unclosed_attrs() {
    let e = error("a(href=/x");
    assert!(e.contains("unclosed attribute list"), "got: {e}");
}

#[test]
fn error_doctype_arguments() {
    let e = error("doctype strict");
    assert!(e.contains("doctype"), "got: {e}");
}

#[test]
fn error_dedent_matches_no_open_level() {
    // 4 deep opens a level; dedenting to 2 matches nothing open.
    let e = error("div\n    p \"deep\"\n  p \"ok\"");
    assert!(e.contains("matches no open level"), "got: {e}");
    assert!(e.contains("4 spaces"), "got: {e}");
}

#[test]
fn error_mixed_tabs_and_spaces_same_line() {
    let e = error("div\n \tp \"a\"");
    assert!(e.contains("mixed tabs and spaces"), "got: {e}");
}

#[test]
fn error_tabs_vs_spaces_across_lines() {
    let e = error("div\n\tp \"a\"\n  p \"b\"");
    assert!(e.contains("spaces") && e.contains("tabs"), "got: {e}");
}

// -------------------------------------------- indent stack (SPEC §2, Python model)

#[test]
fn indent_steps_may_differ_between_blocks() {
    // Python-style: each block picks its own step; only alignment must match.
    let src = "div\n    p \"a\"\nul\n  li \"b\"\n";
    assert_eq!(min(src), "<div><p>a</p></div><ul><li>b</li></ul>");
}

#[test]
fn any_deeper_indent_is_one_child_level() {
    assert_eq!(min("div\n        p \"x\"\n"), "<div><p>x</p></div>");
}

#[test]
fn uneven_step_compiles_with_warning() {
    // The silent-misnesting hazard: +1 space nests (per the stack rule) but warns.
    let out = fhtml::compile_full("div\n  p \"a\"\n   p \"b\"\n", Mode::Min).unwrap();
    assert_eq!(out.html, "<div><p>a<p>b</p></p></div>");
    assert_eq!(out.warnings.len(), 1, "warnings: {:?}", out.warnings);
    assert!(
        out.warnings[0].contains("indent step"),
        "got: {}",
        out.warnings[0]
    );
}

#[test]
fn consistent_files_warn_nothing() {
    let src = "div\n  p \"a\"\n  ul\n    li \"b\"\n";
    let out = fhtml::compile_full(src, Mode::Min).unwrap();
    assert!(out.warnings.is_empty(), "warnings: {:?}", out.warnings);
}

#[test]
fn attr_shaped_class_token_compiles_with_warning() {
    // The top DOM-corruption hazard in generated fhtml: an attribute
    // written as a bare token becomes a class, verbatim (SPEC §3) — so it
    // compiles, but warns. Tailwind's `=` syntax carries `[`/`]`/`:`.
    let out = fhtml::compile_full(
        "div aria-hidden=true absolute > span role=status\n",
        Mode::Min,
    )
    .unwrap();
    assert_eq!(
        out.html,
        "<div class=\"aria-hidden=true absolute\"><span class=\"role=status\"></span></div>"
    );
    assert_eq!(out.warnings.len(), 2, "warnings: {:?}", out.warnings);
    assert!(
        out.warnings[0].contains("`div(aria-hidden=true)`"),
        "got: {}",
        out.warnings[0]
    );
    assert!(
        out.warnings[1].contains("`span(role=status)`"),
        "got: {}",
        out.warnings[1]
    );
    let out = fhtml::compile_full(
        "div data-[state=open]:flex w-1/2 supports-[display=grid]:grid\n",
        Mode::Min,
    )
    .unwrap();
    assert!(out.warnings.is_empty(), "warnings: {:?}", out.warnings);
}

// --------------------------------------------- concat-class lint §9.1

/// Returns the parse warnings for a template source. Goes through the JS
/// target because the lint fires at parse time and this path never
/// evaluates — null data would make some `+` fixtures render errors.
fn lint_warnings(src: &str) -> Vec<String> {
    fhtml::compile_to_js(src, Mode::Min).unwrap().warnings
}

#[test]
fn concat_class_token_warns() {
    // SPEC §9.1 lint: a class name built by `+` is invisible to Tailwind's
    // static scanner. The expression still renders.
    let w = lint_warnings("span {\"bg-\" + color + \"-100\"} \"x\"\n");
    assert_eq!(w.len(), 1, "warnings: {w:?}");
    assert!(
        w[0].starts_with("1:6: warning:") && w[0].contains("string concatenation"),
        "got: {}",
        w[0]
    );
    assert!(
        w[0].contains("`{\"bg-\" + color + \"-100\"}`"),
        "got: {}",
        w[0]
    );
}

#[test]
fn concat_in_class_attr_value_warns() {
    // Both `class` attribute spellings reach the same class list.
    let w = lint_warnings("div(class=\"a {p + q}\")\n");
    assert_eq!(w.len(), 1, "warnings: {w:?}");
    assert!(w[0].contains("string concatenation"), "got: {}", w[0]);
    let w = lint_warnings("div(class={p + q})\n");
    assert_eq!(w.len(), 1, "warnings: {w:?}");
}

#[test]
fn whole_token_class_interp_is_silent() {
    // The documented idiom (SPEC §9.2) stays warning-free.
    let w = lint_warnings("span {active ? \"bg-blue-600\" : \"bg-gray-100\"} \"x\"\n");
    assert!(w.is_empty(), "warnings: {w:?}");
    let w = lint_warnings("span {tone} \"x\"\n");
    assert!(w.is_empty(), "warnings: {w:?}");
}

#[test]
fn concat_outside_class_position_is_silent() {
    // Position-scoped: string building in text and non-class attributes is
    // legitimate and Tailwind-irrelevant.
    let w = lint_warnings("p \"total: {a + b}\"\n");
    assert!(w.is_empty(), "warnings: {w:?}");
    let w = lint_warnings("a(href={base + path}) \"go\"\n");
    assert!(w.is_empty(), "warnings: {w:?}");
    let w = lint_warnings("p\n  | sum {a + b}\n");
    assert!(w.is_empty(), "warnings: {w:?}");
}

#[test]
fn concat_class_on_chain_target_warns() {
    let w = lint_warnings("li > a {\"text-\" + tone} \"x\"\n");
    assert_eq!(w.len(), 1, "warnings: {w:?}");
}

#[test]
fn concat_class_in_statement_body_warns_and_js_target_compiles() {
    // Elements nested under statements pass the same lint; the JS target
    // carries the warning in Output and still emits the module (warnings
    // are compile-time only — nothing lands in the emitted code).
    let out = fhtml::compile_to_js(
        "for item in items\n  li {\"bg-\" + item.tone} \"x\"\n",
        Mode::Min,
    )
    .unwrap();
    assert_eq!(out.warnings.len(), 1, "warnings: {:?}", out.warnings);
    assert!(!out.html.contains("warning"), "module: {}", out.html);
}

#[test]
fn deny_warnings_flips_the_exit_code() {
    // `--deny-warnings` (SPEC §9.1 note): warnings still print, the run
    // fails, and no output is written.
    use std::io::Write;
    use std::process::{Command, Stdio};
    let run = |args: &[&str]| {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fhtml"))
            .args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let src = "span {\"bg-\" + color} \"x\"\n";
        child
            .stdin
            .take()
            .unwrap()
            .write_all(src.as_bytes())
            .unwrap();
        child.wait_with_output().unwrap()
    };
    let ok = run(&[]);
    assert!(ok.status.success(), "plain compile must pass");
    assert!(!ok.stdout.is_empty(), "html still emitted");
    let denied = run(&["--deny-warnings"]);
    assert!(!denied.status.success(), "--deny-warnings must fail");
    let stderr = String::from_utf8_lossy(&denied.stderr);
    assert!(stderr.contains("string concatenation"), "got: {stderr}");
    assert!(stderr.contains("--deny-warnings"), "got: {stderr}");
    assert!(denied.stdout.is_empty(), "no output under deny");
}

// ------------------------------------------------------------------ fmt

#[test]
fn fmt_canonicalizes_indent_div_and_classes() {
    let src = "div flex\n    p(class=\"x y\") text-gray-500 \"hi\"\n";
    let expected = ". flex\n  p x y text-gray-500 \"hi\"\n";
    assert_eq!(fhtml::format(src).unwrap(), expected);
}

#[test]
fn fmt_roundtrip_preserves_output() {
    let src = r#"doctype html
div flex items-center gap-4
	img(src=/img/ava.jpg alt="Erin's avatar") size-12 rounded-full
	.
		p text-lg "Erin Lindford"
		li > a(href=/docs) hover:underline "Docs"
"#;
    let formatted = fhtml::format(src).unwrap();
    assert_eq!(min(&formatted), min(src), "formatted:\n{formatted}");
    // Canonical output is stable: formatting twice changes nothing.
    assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
}

#[test]
fn class_with_quote_is_escaped_in_output() {
    // A `"` inside a class must not end the emitted attribute.
    assert_eq!(
        min(r#"div(class="x\"y")"#),
        r#"<div class="x&quot;y"></div>"#
    );
}

#[test]
fn fmt_hostile_classes_ride_in_class_attr() {
    // `#b` printed bare would reparse as an id token — it must stay quoted.
    let src = "div(class=\"a #b\") c\n";
    let formatted = fhtml::format(src).unwrap();
    assert_eq!(formatted, ".(class=\"a #b c\")\n");
    assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    assert_eq!(min(&formatted), min(src));
}

#[test]
fn fmt_preserves_silent_comments_and_text_blocks() {
    let src = "// keep me\np text-sm\n  | line \"one\"\n  | line two\n";
    let formatted = fhtml::format(src).unwrap();
    assert!(formatted.contains("// keep me"), "got:\n{formatted}");
    assert!(formatted.contains("| line \"one\""), "got:\n{formatted}");
    assert_eq!(min(&formatted), min(src));
}

#[test]
fn fmt_preserves_raw_text_bodies() {
    // SPEC §6.3 guard: content bytes reprint verbatim — indentation inside
    // the `|` untouched, no `\{` escape rewriting, blank `|` lines kept.
    let src = "div\n    script\n        | if (a && b < c) { go(); }\n        |   deep(\\{);\n        |\n        | done();\n";
    let formatted = fhtml::format(src).unwrap();
    assert_eq!(
        formatted,
        ".\n  script\n    | if (a && b < c) { go(); }\n    |   deep(\\{);\n    |\n    | done();\n"
    );
    assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    assert_eq!(min(&formatted), min(src));
}

#[test]
fn error_reserved_word_as_tag_with_attrs() {
    let e = error("if(x=1) flex");
    assert!(e.contains("reserved word"), "got: {e}");
}

#[test]
fn error_positions_are_reported() {
    let err = compile("div\n  p \"unclosed", Mode::Min).unwrap_err();
    assert_eq!(err.line, 2);
}

// ------------------------------------- expression mini-language §9.3/§9.4

mod expr {
    use fhtml::expr::{deep_eq, eval, parse, stringify, Scope, Value};

    /// Evaluates `src` against a data map built from `data` pairs, null ctx.
    fn ev_with(src: &str, data: Value) -> Result<Value, String> {
        let e = parse(src).map_err(|e| e.to_string())?;
        eval(&e, &Scope::new(&data), &Value::Null).map_err(|e| e.to_string())
    }

    fn ev(src: &str) -> Value {
        ev_with(src, sample_data()).unwrap()
    }

    fn ev_err(src: &str) -> String {
        ev_with(src, sample_data()).unwrap_err()
    }

    /// Interpolation-style result: eval + stringify.
    fn show(src: &str) -> String {
        stringify(&ev(src)).unwrap()
    }

    fn map(pairs: &[(&str, Value)]) -> Value {
        Value::Map(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn sample_data() -> Value {
        map(&[
            ("n", Value::from(3.0)),
            ("s", Value::from("hi")),
            ("flag", Value::from(true)),
            ("items", Value::from(vec![10i64, 20, 30])),
            (
                "user",
                map(&[("name", Value::from("Erin")), ("age", Value::from(29i64))]),
            ),
        ])
    }

    // -------------------------------------------------- §9.3 precedence

    #[test]
    fn precedence_mult_over_add() {
        assert_eq!(ev("1 + 2 * 3"), Value::Number(7.0));
        assert_eq!(ev("(1 + 2) * 3"), Value::Number(9.0));
    }

    #[test]
    fn precedence_add_over_compare_over_equality() {
        // `1 + 1 < 3 == true` = ((1+1) < 3) == true
        assert_eq!(ev("1 + 1 < 3 == true"), Value::Bool(true));
    }

    #[test]
    fn precedence_and_binds_tighter_than_or() {
        assert_eq!(ev("false || true && false"), Value::Bool(false));
        assert_eq!(ev("true || false && false"), Value::Bool(true));
    }

    #[test]
    fn precedence_ternary_is_lowest_and_right_associative() {
        assert_eq!(ev("0 ? 'a' : 1 ? 'b' : 'c'"), Value::from("b"));
        assert_eq!(ev("flag || false ? 'y' : 'n'"), Value::from("y"));
    }

    #[test]
    fn precedence_unary_and_postfix() {
        assert_eq!(ev("-user.age"), Value::Number(-29.0));
        assert_eq!(ev("!user.age"), Value::Bool(false));
        assert_eq!(ev("2 - -3"), Value::Number(5.0));
        assert_eq!(ev("!!s"), Value::Bool(true));
    }

    #[test]
    fn postfix_paths_and_indexing() {
        assert_eq!(ev("user.name"), Value::from("Erin"));
        assert_eq!(ev("items[0]"), Value::Number(10.0));
        assert_eq!(ev("items[1 + 1]"), Value::Number(30.0));
        assert_eq!(ev("user['name']"), Value::from("Erin"));
    }

    #[test]
    fn logical_operators_yield_operand_values() {
        // `||`/`&&` short-circuit and yield the deciding operand, enabling
        // `{name || 'anonymous'}` defaults.
        assert_eq!(ev("null || 'default'"), Value::from("default"));
        assert_eq!(ev("s || 'default'"), Value::from("hi"));
        assert_eq!(ev("s && 'next'"), Value::from("next"));
        assert_eq!(ev("0 && 'next'"), Value::Number(0.0));
    }

    #[test]
    fn short_circuit_skips_errors_in_untaken_branch() {
        assert_eq!(ev("false && 1 / 0 == 1"), Value::Bool(false));
        assert_eq!(ev("true || 1 / 0 == 1"), Value::Bool(true));
        assert_eq!(ev("flag ? 'ok' : 1 / 0"), Value::from("ok"));
    }

    // ------------------------------------------------ §9.3 parse errors

    #[test]
    fn parse_errors_carry_byte_offsets() {
        assert_eq!(parse("").unwrap_err().offset, 0);
        assert_eq!(parse("1 +").unwrap_err().offset, 3);
        // trailing garbage after a complete expression
        let e = parse("a b").unwrap_err();
        assert_eq!(e.offset, 2);
        assert!(e.msg.contains("unexpected `b`"), "got: {}", e.msg);
        assert!(parse("a ? b").unwrap_err().msg.contains("`:`"));
        assert!(parse("x[1").unwrap_err().msg.contains("`]`"));
        assert!(parse("a.").unwrap_err().msg.contains("name after `.`"));
        assert!(parse("'unclosed").unwrap_err().msg.contains("unclosed"));
        assert!(parse("(1 + 2").unwrap_err().msg.contains("`)`"));
    }

    #[test]
    fn no_host_language_escape() {
        // The grammar is closed: no calls, no assignment.
        assert!(parse("foo(1)").is_err());
        assert!(parse("a = 1").is_err());
        assert!(parse("a | b").is_err());
    }

    #[test]
    fn string_literals_and_escapes() {
        assert_eq!(ev("'it\\'s'"), Value::from("it's"));
        assert_eq!(ev(r#""a\"b""#), Value::from("a\"b"));
        assert_eq!(ev(r"'a\\b'"), Value::from("a\\b"));
        assert!(parse(r"'\n'").unwrap_err().msg.contains("unknown escape"));
    }

    #[test]
    fn number_literals() {
        assert_eq!(ev("42"), Value::Number(42.0));
        assert_eq!(ev("2.5"), Value::Number(2.5));
        assert_eq!(ev("1e3"), Value::Number(1000.0));
        assert_eq!(ev("1.5e-2"), Value::Number(0.015));
    }

    // -------------------------------------------------- §9.4 falsiness

    #[test]
    fn falsiness_table() {
        for falsy in ["null", "false", "0", "''", "missing"] {
            assert_eq!(
                ev(&format!("{falsy} ? 1 : 0")),
                Value::Number(0.0),
                "{falsy}"
            );
        }
        let empties = map(&[("l", Value::List(vec![])), ("m", Value::Map(vec![]))]);
        assert_eq!(
            ev_with("l ? 1 : 0", empties.clone()).unwrap(),
            Value::Number(0.0)
        );
        assert_eq!(ev_with("m ? 1 : 0", empties).unwrap(), Value::Number(0.0));
        for truthy in ["true", "1", "0.5", "-1", "'0'", "' '", "items", "user"] {
            assert_eq!(
                ev(&format!("{truthy} ? 1 : 0")),
                Value::Number(1.0),
                "{truthy}"
            );
        }
    }

    // ---------------------------------------------- §9.4 deep equality

    #[test]
    fn equality_is_deep_and_structural() {
        let data = map(&[
            ("a", Value::from(vec![1i64, 2])),
            ("b", Value::from(vec![1i64, 2])),
            ("c", Value::from(vec![1i64, 3])),
            (
                "m1",
                map(&[("x", Value::from(1i64)), ("y", Value::from(2i64))]),
            ),
            (
                "m2",
                map(&[("y", Value::from(2i64)), ("x", Value::from(1i64))]),
            ),
        ]);
        assert_eq!(ev_with("a == b", data.clone()).unwrap(), Value::Bool(true));
        assert_eq!(ev_with("a == c", data.clone()).unwrap(), Value::Bool(false));
        assert_eq!(ev_with("a != c", data.clone()).unwrap(), Value::Bool(true));
        // maps compare by key set, not insertion order
        assert_eq!(ev_with("m1 == m2", data).unwrap(), Value::Bool(true));
    }

    #[test]
    fn equality_never_coerces_across_types() {
        assert_eq!(ev("null == 0"), Value::Bool(false));
        assert_eq!(ev("'1' == 1"), Value::Bool(false));
        assert_eq!(ev("true == 1"), Value::Bool(false));
        assert_eq!(ev("'' == null"), Value::Bool(false));
        assert!(deep_eq(&Value::Null, &Value::Null));
    }

    // ------------------------------------------- §9.4 `+` coercion matrix

    #[test]
    fn plus_adds_numbers_and_concats_strings() {
        assert_eq!(ev("1 + 2"), Value::Number(3.0));
        assert_eq!(ev("'#' + 7"), Value::from("#7"));
        assert_eq!(ev("7 + '#'"), Value::from("7#"));
        assert_eq!(ev("'a' + 'b'"), Value::from("ab"));
        assert_eq!(ev("'x=' + null"), Value::from("x="));
        assert_eq!(ev("'x=' + true"), Value::from("x=true"));
    }

    #[test]
    fn plus_rejects_other_combinations() {
        assert!(ev_err("null + 1").contains("`+`"));
        assert!(ev_err("true + 1").contains("`+`"));
        assert!(ev_err("'a' + items").contains("list"));
        assert!(ev_err("user + 'a'").contains("map"));
        assert!(ev_err("items + items").contains("`+`"));
    }

    #[test]
    fn other_arithmetic_and_comparison_require_numbers() {
        assert!(ev_err("1 - '2'").contains("requires numbers"));
        assert!(ev_err("'a' < 'b'").contains("requires numbers"));
        assert!(ev_err("s * 2").contains("requires numbers"));
        assert!(ev_err("-s").contains("requires a number"));
        assert_eq!(ev_err("1 / 0"), "division by zero");
        assert_eq!(ev_err("1 % 0"), "modulo by zero");
    }

    // -------------------------------------- §9.4 missing paths are null

    #[test]
    fn missing_paths_keys_and_indexes_are_null() {
        for miss in [
            "missing",
            "user.missing",
            "user.missing.deeper",
            "missing.x",
            "items[99]",
            "items[-1]",
            "items[0.5]",
            "items['x']",
            "user[0]",
            "n.field",
            "s[0]",
            "null.x",
        ] {
            assert_eq!(ev(&format!("{miss} == null")), Value::Bool(true), "{miss}");
        }
    }

    // ------------------------------------------------- §9.4 ctx root

    #[test]
    fn ctx_resolves_in_every_scope_and_cannot_be_shadowed() {
        let e = parse("ctx.theme").unwrap();
        let ctx = map(&[("theme", Value::from("dark"))]);
        // a data key named `ctx` does not shadow the host ctx
        let data = map(&[("ctx", Value::from("decoy"))]);
        let mut scope = Scope::new(&data);
        assert_eq!(eval(&e, &scope, &ctx).unwrap(), Value::from("dark"));
        // nor does a local binding (the renderer also rejects `for ctx in …`
        // at parse time — covered when it lands; TODO)
        scope.push("ctx", Value::from("decoy2"));
        assert_eq!(eval(&e, &scope, &ctx).unwrap(), Value::from("dark"));
    }

    #[test]
    fn scope_locals_shadow_data_innermost_first() {
        let data = map(&[("x", Value::from(1i64))]);
        let e = parse("x").unwrap();
        let mut scope = Scope::new(&data);
        scope.push("x", Value::from(2i64));
        scope.push("x", Value::from(3i64));
        assert_eq!(eval(&e, &scope, &Value::Null).unwrap(), Value::Number(3.0));
        scope.pop();
        assert_eq!(eval(&e, &scope, &Value::Null).unwrap(), Value::Number(2.0));
        scope.pop();
        assert_eq!(eval(&e, &scope, &Value::Null).unwrap(), Value::Number(1.0));
    }

    // --------------------------------------------- §9.4 stringification

    #[test]
    fn stringification_rules() {
        assert_eq!(show("null"), "");
        assert_eq!(show("missing"), "");
        assert_eq!(show("true"), "true");
        assert_eq!(show("false"), "false");
        assert_eq!(show("'hi'"), "hi");
    }

    #[test]
    fn number_stringification_is_shortest_round_trip_without_exponent() {
        assert_eq!(show("3"), "3");
        assert_eq!(show("3.0"), "3");
        assert_eq!(show("2.5"), "2.5");
        assert_eq!(show("0.1"), "0.1");
        assert_eq!(show("-0"), "0");
        assert_eq!(show("1e21"), "1000000000000000000000");
        assert_eq!(show("1e-7"), "0.0000001");
        assert_eq!(show("1 / 3"), (1.0f64 / 3.0).to_string());
    }

    #[test]
    fn lists_and_maps_do_not_stringify() {
        assert!(stringify(&ev("items"))
            .unwrap_err()
            .to_string()
            .contains("list"));
        assert!(stringify(&ev("user"))
            .unwrap_err()
            .to_string()
            .contains("map"));
    }
}

// ---------------------------------- template-layer parsing §9.1–§9.2, §10

mod template_parse {
    use fhtml::{compile_opts, Mode, Options};

    /// Parses via the static path; template files fail with the static-only
    /// message, so use `parses` to assert the *parse* succeeded.
    fn error(src: &str) -> String {
        compile_opts(src, &Options::default())
            .unwrap_err()
            .to_string()
    }

    fn parses(src: &str) {
        match compile_opts(src, &Options::default()) {
            Ok(_) => {}
            Err(e) => assert!(
                e.to_string().contains("static"),
                "parse error in {src:?}: {e}"
            ),
        }
    }

    fn no_templates_error(src: &str) -> String {
        compile_opts(
            src,
            &Options {
                mode: Mode::Min,
                templates: false,
                ..Default::default()
            },
        )
        .unwrap_err()
        .to_string()
    }

    // ------------------------------------------ §10.1 if / elif / else

    #[test]
    fn if_chain_parses() {
        parses("if user\n  p \"a\"\nelif invited\n  p \"b\"\nelse\n  p \"c\"\n");
        // blank lines between chain parts are fine
        parses("if user\n  p \"a\"\n\nelse\n  p \"c\"\n");
    }

    #[test]
    fn error_elif_without_if() {
        let e = error("elif user\n  p \"x\"");
        assert!(e.contains("directly follow"), "got: {e}");
    }

    #[test]
    fn error_else_without_if() {
        let e = error("else\n  p \"x\"");
        assert!(e.contains("directly follow"), "got: {e}");
    }

    #[test]
    fn error_sibling_between_if_and_elif() {
        let e = error("if user\n  p \"a\"\np \"between\"\nelif x\n  p \"b\"");
        assert!(e.contains("directly follow"), "got: {e}");
    }

    #[test]
    fn error_elif_at_wrong_indent() {
        let e = error("div\n  if user\n    p \"a\"\nelif x\n  p \"b\"");
        assert!(e.contains("directly follow"), "got: {e}");
    }

    #[test]
    fn error_else_takes_no_condition() {
        let e = error("if user\n  p \"a\"\nelse invited\n  p \"b\"");
        assert!(e.contains("no condition"), "got: {e}");
    }

    #[test]
    fn error_statement_needs_block() {
        let e = error("if user\np \"next\"");
        assert!(e.contains("indented block"), "got: {e}");
        let e = error("for x in items\np \"next\"");
        assert!(e.contains("indented block"), "got: {e}");
    }

    #[test]
    fn error_if_needs_expression() {
        let e = error("if\n  p \"x\"");
        assert!(e.contains("needs an expression"), "got: {e}");
    }

    #[test]
    fn error_bad_condition_reports_expression_position() {
        // `1 +` — the expression error lands after the `+`
        let e = error("if 1 +\n  p \"x\"");
        assert!(e.starts_with("1:7:"), "got: {e}");
    }

    // ------------------------------------------------ §10.2 for / empty

    #[test]
    fn for_forms_parse() {
        parses("for item in items\n  p \"{item}\"\n");
        parses("for item, i in items\n  p \"{i}: {item}\"\n");
        parses("for item in items\n  p \"{item}\"\nempty\n  p \"none\"\n");
    }

    #[test]
    fn error_empty_without_for() {
        let e = error("empty\n  p \"x\"");
        assert!(e.contains("directly follow"), "got: {e}");
    }

    #[test]
    fn error_empty_takes_nothing() {
        let e = error("for x in items\n  p \"a\"\nempty handed\n  p \"b\"");
        assert!(e.contains("`empty` takes nothing"), "got: {e}");
    }

    #[test]
    fn error_for_missing_in() {
        let e = error("for item items\n  p \"x\"");
        assert!(e.contains("expected `in`"), "got: {e}");
    }

    #[test]
    fn error_for_missing_variable() {
        let e = error("for\n  p \"x\"");
        assert!(e.contains("loop variable"), "got: {e}");
    }

    #[test]
    fn error_for_cannot_bind_ctx_or_literals() {
        let e = error("for ctx in items\n  p \"x\"");
        assert!(e.contains("cannot be shadowed"), "got: {e}");
        let e = error("for x, ctx in items\n  p \"x\"");
        assert!(e.contains("cannot be shadowed"), "got: {e}");
        let e = error("for true in items\n  p \"x\"");
        assert!(e.contains("literal"), "got: {e}");
    }

    #[test]
    fn error_for_duplicate_names() {
        let e = error("for x, x in items\n  p \"x\"");
        assert!(e.contains("must differ"), "got: {e}");
    }

    // ------------------------------------------- §9.1–§9.2 interpolation

    #[test]
    fn interpolation_contexts_parse() {
        parses("p \"Hi, {user.name}\"\n");
        parses("p\n  | total: {n}\n  | raw: {!html}\n");
        parses("a(href={user.url} title=\"Profile of {user.name}\")\n");
        parses("button px-4 {active ? 'bg-blue-600' : 'bg-gray-100'} \"Go\"\n");
        parses("div(class=\"a {x} b\")\n");
    }

    #[test]
    fn error_raw_bare_token_is_class_position() {
        // SPEC §9.1: class position is not a content position. (The raw-HTML
        // idiom is a `| {!expr}` text-block line — the spec's inline
        // example predates this ruling; flagged for a later doc pass.)
        let e = error("article prose {!post.html}");
        assert!(e.contains("class position"), "got: {e}");
        assert!(e.contains("| {!expr}"), "got: {e}");
    }

    #[test]
    fn error_raw_in_quoted_attr() {
        let e = error("a(title=\"x {!evil} y\")");
        assert!(e.contains("forbidden inside attribute"), "got: {e}");
    }

    #[test]
    fn error_raw_in_unquoted_attr() {
        let e = error("a(href={!evil})");
        assert!(e.contains("forbidden inside attribute"), "got: {e}");
    }

    #[test]
    fn error_raw_in_class_position() {
        let e = error("div {!classes}");
        assert!(e.contains("class position"), "got: {e}");
    }

    #[test]
    fn error_unquoted_attr_expr_must_be_whole_value() {
        let e = error("a(href={base}/path)");
        assert!(e.contains("entire attribute value"), "got: {e}");
    }

    #[test]
    fn error_glued_class_interpolation() {
        // `{` mid-token is inert (SPEC §4.2: tokens classify by leading char
        // only) — `bg-{color}-100` stays one literal class, static-compatible.
        assert_eq!(
            fhtml::compile("div bg-{color}-100", Mode::Min).unwrap(),
            "<div class=\"bg-{color}-100\"></div>"
        );
        // but a token *starting* with `{` must be the whole token:
        let e = error("div {color}-100");
        assert!(e.contains("whole token"), "got: {e}");
        // and inside class="…" interpolation must be whitespace-separated:
        let e = error("div(class=\"bg-{color}-100\")");
        assert!(e.contains("whitespace-separated"), "got: {e}");
    }

    #[test]
    fn error_unclosed_interpolation() {
        let e = error("p \"hi {name\"");
        assert!(e.contains("unclosed"), "got: {e}");
    }

    #[test]
    fn expression_strings_may_contain_braces_and_quotes() {
        parses("p \"{open ? '{' : '}'}\"\n");
        parses("p \"{a == \"x\" ? 'y' : 'z'}\"\n");
    }

    #[test]
    fn escaped_brace_is_literal() {
        // static behavior preserved: `\{` is a literal brace, no interpolation.
        assert_eq!(
            fhtml::compile("p \"set \\{x}\"", Mode::Min).unwrap(),
            "<p>set {x}</p>"
        );
    }

    // ------------------------------------- §10.3 def / children (parse layer)

    #[test]
    fn def_parses_with_params_defaults_and_children() {
        // Defaults are expressions: string, number, boolean, braced (§10.3).
        parses("def alert(kind='info' compact=false max=3)\n  p \"{kind}\"\n  children\n");
        parses("def footer\n  p \"x\"\n"); // zero params, parens optional
        parses("def footer()\n  p \"x\"\n");
        parses("def card(limit={ctx.pageSize - 1})\n  p \"{limit}\"\n");
    }

    #[test]
    fn def_forward_reference_parses() {
        // Definition order doesn't matter (§10.3): the call comes first.
        parses("+card(title='x')\ndef card(title)\n  p \"{title}\"\n");
    }

    #[test]
    fn error_def_only_at_top_level() {
        let e = error("div p-4\n  def inner(x)\n    p \"x\"");
        assert!(e.contains("top level"), "got: {e}");
        let e = error("if flag\n  def inner(x)\n    p \"x\"");
        assert!(e.contains("top level"), "got: {e}");
        let e = error("def outer(x)\n  def inner(y)\n    p \"y\"");
        assert!(e.contains("top level"), "got: {e}");
    }

    #[test]
    fn error_def_redefinition() {
        let e = error("def card(a)\n  p \"1\"\ndef card(b)\n  p \"2\"");
        assert!(e.contains("already defined"), "got: {e}");
        assert!(e.contains("line 1"), "got: {e}");
    }

    #[test]
    fn error_def_shape() {
        let e = error("def\n  p \"x\"");
        assert!(e.contains("component name"), "got: {e}");
        let e = error("def if(x)\n  p \"x\"");
        assert!(e.contains("reserved word"), "got: {e}");
        let e = error("def card(title) p-4\n  p \"x\"");
        assert!(e.contains("after the parameter list"), "got: {e}");
        let e = error("def card(title)");
        assert!(e.contains("needs an indented block"), "got: {e}");
        let e = error("def card(title\n  p \"x\"");
        assert!(e.contains("unclosed parameter list"), "got: {e}");
    }

    #[test]
    fn error_def_params() {
        let e = error("def card(a a)\n  p \"x\"");
        assert!(e.contains("duplicate parameter"), "got: {e}");
        let e = error("def card(ctx)\n  p \"x\"");
        assert!(e.contains("cannot be shadowed"), "got: {e}");
        let e = error("def card(true)\n  p \"x\"");
        assert!(e.contains("expression literal"), "got: {e}");
        let e = error("def card(a=)\n  p \"x\"");
        assert!(e.contains("missing default"), "got: {e}");
        // A spaced default needs braces (§10.3).
        let e = error("def card(a=x + y)\n  p \"x\"");
        assert!(e.contains("braces"), "got: {e}");
    }

    #[test]
    fn error_kebab_case_names_suggest_underscores() {
        // The top LLM-written mistake: names live in the expression grammar,
        // where `-` is minus — the error suggests the underscore fix
        // (§10.3/§10.4).
        let e = error("def blog-post(x)\n  p \"{x}\"");
        assert!(e.contains("1:5"), "got: {e}");
        assert!(e.contains("`def blog_post(…)`"), "got: {e}");
        let e = error("def card(img-src)\n  p \"x\"");
        assert!(e.contains("`img_src`"), "got: {e}");
        let e = error("+blog-post(x=1)");
        assert!(e.contains("`+blog_post(…)`"), "got: {e}");
        let e = error("def card(a)\n  p \"{a}\"\n+card(img-src=1)");
        assert!(e.contains("`img_src`"), "got: {e}");
    }

    #[test]
    fn error_children_placement() {
        let e = error("children");
        assert!(e.contains("inside a `def` body"), "got: {e}");
        let e = error("div p-4\n  children");
        assert!(e.contains("inside a `def` body"), "got: {e}");
        let e = error("def card(x)\n  children now");
        assert!(e.contains("takes nothing"), "got: {e}");
    }

    #[test]
    fn children_inside_def_statements_parses() {
        // The def body is the scope, however deeply nested (§10.3).
        parses("def card(x)\n  if x\n    children\n  else\n    p \"none\"\n");
    }

    // ------------------------------------------ §10.4 +call (parse layer)

    #[test]
    fn call_parses_all_argument_shapes() {
        // bare = true, quoted = string (with interpolation), unquoted =
        // expression, braced = spaced expression (§10.4).
        parses("+card(title=\"Monthly {kind}\" compact count=3 show=false user=member.profile n={a + b})\n");
        parses("+card\n"); // legal when every param has a default (§10.4)
        parses("+card(title='x')\n  p text-sm \"the children block\"\n");
    }

    #[test]
    fn error_call_shape() {
        let e = error("+ card(title='x')");
        assert!(e.contains("needs a name"), "got: {e}");
        let e = error("+card(title='x') p-4");
        assert!(e.contains("after the component call"), "got: {e}");
        let e = error("+card(title='x'");
        assert!(e.contains("unclosed argument list"), "got: {e}");
    }

    #[test]
    fn error_call_args() {
        let e = error("+card(a=1 a=2)");
        assert!(e.contains("duplicate argument"), "got: {e}");
        let e = error("+card(a=)");
        assert!(e.contains("missing value"), "got: {e}");
        // Unquoted values are expressions — a bare path must be quoted (§10.4).
        let e = error("+card(href=/blog/x)");
        assert!(e.contains("quote a string"), "got: {e}");
    }

    #[test]
    fn error_call_cannot_be_chain_target() {
        // §10.4: `>` chains single elements; a call is not an element.
        let e = error("li > +card(title='x')");
        assert!(e.contains("cannot be the target"), "got: {e}");
    }

    #[test]
    fn no_templates_rejects_components() {
        let e = no_templates_error("def card(title)\n  p \"x\"");
        assert!(e.contains("--no-templates"), "got: {e}");
        let e = no_templates_error("+card(title='x')");
        assert!(e.contains("--no-templates"), "got: {e}");
        let e = no_templates_error("children");
        assert!(e.contains("--no-templates"), "got: {e}");
    }

    // ------------------------------------------------- §9.2 --no-templates

    #[test]
    fn no_templates_rejects_all_constructs() {
        let e = no_templates_error("if user\n  p \"x\"");
        assert!(e.contains("--no-templates"), "got: {e}");
        let e = no_templates_error("p \"hi {name}\"");
        assert!(e.contains("--no-templates"), "got: {e}");
        let e = no_templates_error("div {active}");
        assert!(e.contains("--no-templates"), "got: {e}");
        let e = no_templates_error("a(href={x})");
        assert!(e.contains("--no-templates"), "got: {e}");
        let e = no_templates_error("p\n  | count: {n}");
        assert!(e.contains("--no-templates"), "got: {e}");
    }

    #[test]
    fn no_templates_still_compiles_p0() {
        let out = compile_opts(
            "p \"escaped \\{brace}\"",
            &Options {
                mode: Mode::Min,
                templates: false,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(out.html, "<p>escaped {brace}</p>");
    }

    // ------------------------------------------------ fmt on template files

    #[test]
    fn fmt_reprints_statements_and_interpolation() {
        let src = "if user\n    p   text-sm \"Hi, { user.name }\"\nelif invited\n    p \"{'#' + n}\"\nelse\n    a(href=/login class=\"x {cls} y\") \"Sign in\"\nfor item,   i in items\n    li \"{i}: {item.title}\"\nempty\n    li \"none\"\n";
        let formatted = fhtml::format(src).unwrap();
        let expected = "if user\n  p text-sm \"Hi, {user.name}\"\nelif invited\n  p \"{'#' + n}\"\nelse\n  a(href=/login) x {cls} y \"Sign in\"\nfor item, i in items\n  li \"{i}: {item.title}\"\nempty\n  li \"none\"\n";
        assert_eq!(formatted, expected);
        // idempotent
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    }

    #[test]
    fn fmt_preserves_raw_interpolation_and_text_blocks() {
        let src =
            "article prose\n  | {!post.html}\np\n  | total: {n} items\n  | raw: {!html} \\{lit}\n";
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(formatted, src);
    }

    #[test]
    fn fmt_guards_interpolations_starting_with_bang() {
        // `{ !a}` is Not-a; printed as `{!a}` it would reparse as *raw*
        // interpolation of `a` (SPEC §9.1) — different output in text, a
        // parse error in attributes and classes. fmt keeps one space.
        let src = "p \"{  !a  }\"\np(hidden={  !a  }) x { !a }\n  | { !a }\n";
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(
            formatted,
            "p \"{ !a}\"\np(hidden={ !a}) x { !a}\n  | { !a}\n"
        );
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
        let d = fhtml::json::parse(r#"{"a": false}"#).unwrap();
        assert_eq!(
            fhtml::render(&formatted, &d, Mode::Min).unwrap(),
            fhtml::render(src, &d, Mode::Min).unwrap()
        );
    }

    // ----------------------------------------- fmt on components §10.3–§10.4

    #[test]
    fn fmt_reprints_defs_calls_and_children() {
        // Params and args normalize to single spaces; defaults and expression
        // values print bare when the reparse is identical and braced when
        // spaced; string arguments always stay quoted (bare would reparse as
        // an expression, SPEC §10.4); bodies indent one step.
        let src = "def card(  title   n={ i + 1 }  compact={1}  )\n    h3 \"{title}\"\n    children\n+card( title=\"Hi\"  n={2}   compact )\n      p \"body\"\n+card(title={ !x } n={a + b})\n+card\n";
        let expected = "def card(title n={i + 1} compact=1)\n  h3 \"{title}\"\n  children\n+card(title=\"Hi\" n=2 compact)\n  p \"body\"\n+card(title=!x n={a + b})\n+card\n";
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(formatted, expected);
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    }

    #[test]
    fn fmt_keeps_defs_in_place() {
        // A definition reprints where it sat — hoisting would detach the
        // comment documenting it and reorder the file.
        let src = "// intro\np \"before\"\n// the card\ndef card\n  p \"c\"\n+card\n";
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(formatted, src);
    }

    #[test]
    fn fmt_component_file_render_is_unchanged() {
        // The fmt invariant extends to components: formatting never changes
        // rendered output, including through `children` and defaults.
        let src = "def item(label done=false)\n  li \"{label}: {done}\"\n    children\nul\n  for t in tasks\n    +item(label=\"{t.name}\" done={ t.done })\n      em \"note\"\n";
        let formatted = fhtml::format(src).unwrap();
        let d = fhtml::json::parse(r#"{"tasks": [{"name": "a", "done": true}, {"name": "b"}]}"#)
            .unwrap();
        assert_eq!(
            fhtml::render(&formatted, &d, Mode::Min).unwrap(),
            fhtml::render(src, &d, Mode::Min).unwrap()
        );
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    }

    #[test]
    fn fmt_blog_cards_def_corpus_is_stable() {
        // The measured demo formats idempotently and still renders
        // byte-identically to its fully expanded twin.
        let src = include_str!("corpus/blog-cards-def.fhtml");
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
        use fhtml::Value;
        assert_eq!(
            fhtml::render(&formatted, &Value::Null, Mode::Min).unwrap(),
            fhtml::render(src, &Value::Null, Mode::Min).unwrap()
        );
    }
}

// ------------------------------------------- rendering §9–§11 (render API)

mod render {
    use fhtml::{render, render_full, Mode, Value};

    fn data(json: &str) -> Value {
        fhtml::json::parse(json).unwrap()
    }

    fn min_with(src: &str, json: &str) -> String {
        render(src, &data(json), Mode::Min).unwrap()
    }

    fn render_err(src: &str, json: &str) -> fhtml::Error {
        render(src, &data(json), Mode::Min).unwrap_err()
    }

    // ------------------------------------------------- §9.1 interpolation

    #[test]
    fn golden_if_chain_from_spec_10_1() {
        let src = r#"if user
  p "Welcome back, {user.name}"
elif invited
  p "Finish signing up"
else
  a(href=/login) "Sign in"
"#;
        assert_eq!(
            min_with(src, r#"{"user": {"name": "Erin"}}"#),
            "<p>Welcome back, Erin</p>"
        );
        assert_eq!(
            min_with(src, r#"{"invited": true}"#),
            "<p>Finish signing up</p>"
        );
        assert_eq!(min_with(src, "{}"), r#"<a href="/login">Sign in</a>"#);
    }

    #[test]
    fn text_interpolation_is_escaped_raw_is_not() {
        let d = r#"{"html": "<b>&amp;</b>"}"#;
        assert_eq!(
            min_with(r#"p "{html}""#, d),
            "<p>&lt;b&gt;&amp;amp;&lt;/b&gt;</p>"
        );
        assert_eq!(min_with("p\n  | {!html}", d), "<p><b>&amp;</b></p>");
        assert_eq!(min_with(r#"p "{!html}""#, d), "<p><b>&amp;</b></p>");
    }

    #[test]
    fn attr_interpolation_escapes_quotes() {
        assert_eq!(
            min_with(
                r#"a(title="Profile of {name}")"#,
                r#"{"name": "\"E\" <&>"}"#
            ),
            r#"<a title="Profile of &quot;E&quot; &lt;&amp;&gt;"></a>"#
        );
    }

    #[test]
    fn unquoted_attr_expr_is_whole_value() {
        assert_eq!(
            min_with(
                "a(href={user.url} tabindex={n})",
                r#"{"user": {"url": "/u/7"}, "n": 3}"#
            ),
            r#"<a href="/u/7" tabindex="3"></a>"#
        );
    }

    #[test]
    fn missing_names_render_empty() {
        assert_eq!(min_with(r#"p "x{missing.path}y""#, "{}"), "<p>xy</p>");
    }

    #[test]
    fn interpolating_a_list_is_an_error_with_position() {
        let e = render_err(r#"p "n: {items}""#, r#"{"items": [1]}"#);
        assert_eq!((e.line, e.col), (1, 7)); // the `{`
        assert!(
            e.msg.contains("cannot interpolate a list"),
            "got: {}",
            e.msg
        );
    }

    // ---------------------------------------------- §9.2 class position

    #[test]
    fn class_interpolation_splits_on_whitespace() {
        let src = "button px-4 {active ? 'bg-blue-600 text-white' : 'bg-gray-100'} \"Go\"";
        assert_eq!(
            min_with(src, r#"{"active": true}"#),
            r#"<button class="px-4 bg-blue-600 text-white">Go</button>"#
        );
        assert_eq!(
            min_with(src, r#"{"active": false}"#),
            r#"<button class="px-4 bg-gray-100">Go</button>"#
        );
    }

    #[test]
    fn empty_class_interpolation_drops_cleanly() {
        assert_eq!(min_with("div {cls}", "{}"), "<div></div>");
        assert_eq!(
            min_with("div {cls}", r#"{"cls": "  a   b "}"#),
            r#"<div class="a b"></div>"#
        );
    }

    #[test]
    fn class_attr_segments_merge_in_order() {
        assert_eq!(
            min_with(r#"div(class="a {x} b") c"#, r#"{"x": "mid"}"#),
            r#"<div class="a mid b c"></div>"#
        );
    }

    #[test]
    fn class_interpolation_result_is_attribute_escaped() {
        assert_eq!(
            min_with("div {cls}", r#"{"cls": "a<b\"c"}"#),
            r#"<div class="a&lt;b&quot;c"></div>"#
        );
    }

    // ------------------------------------------------- §10.2 for / empty

    #[test]
    fn golden_for_loop_from_project_md() {
        let src = r#"ul divide-y divide-gray-100
  for item, i in items
    li py-2 flex justify-between
      span "{i + 1}. {item.title}"
      span text-gray-400 "{item.date}"
  empty
    li py-2 text-gray-400 "Nothing here yet."
"#;
        let d = r#"{"items": [
            {"title": "Ship it", "date": "Jul 6"},
            {"title": "Run bench", "date": "Jul 7"}
        ]}"#;
        let expected = "<ul class=\"divide-y divide-gray-100\">\
<li class=\"py-2 flex justify-between\"><span>1. Ship it</span><span class=\"text-gray-400\">Jul 6</span></li>\
<li class=\"py-2 flex justify-between\"><span>2. Run bench</span><span class=\"text-gray-400\">Jul 7</span></li>\
</ul>";
        assert_eq!(min_with(src, d), expected);
        assert_eq!(
            min_with(src, r#"{"items": []}"#),
            "<ul class=\"divide-y divide-gray-100\"><li class=\"py-2 text-gray-400\">Nothing here yet.</li></ul>"
        );
        // null iterable takes the empty block too
        assert_eq!(
            min_with(src, "{}"),
            "<ul class=\"divide-y divide-gray-100\"><li class=\"py-2 text-gray-400\">Nothing here yet.</li></ul>"
        );
    }

    #[test]
    fn for_over_map_yields_values_and_keys_in_insertion_order() {
        let src = "for v, k in scores\n  p \"{k}={v}\"";
        assert_eq!(
            min_with(src, r#"{"scores": {"b": 2, "a": 1}}"#),
            "<p>b=2</p><p>a=1</p>"
        );
    }

    #[test]
    fn loop_variables_shadow_and_unshadow() {
        let src = "p \"{x}\"\nfor x in xs\n  p \"{x}\"\np \"{x}\"";
        assert_eq!(
            min_with(src, r#"{"x": "outer", "xs": ["a", "b"]}"#),
            "<p>outer</p><p>a</p><p>b</p><p>outer</p>"
        );
    }

    #[test]
    fn nested_loops() {
        let src = "for row in grid\n  for cell in row\n    span \"{cell}\"";
        assert_eq!(
            min_with(src, r#"{"grid": [[1, 2], [3]]}"#),
            "<span>1</span><span>2</span><span>3</span>"
        );
    }

    #[test]
    fn for_over_scalars_is_a_render_error() {
        let e = render_err("for c in word\n  p \"{c}\"", r#"{"word": "abc"}"#);
        assert!(
            e.msg.contains("strings are not character sequences"),
            "got: {}",
            e.msg
        );
        let e = render_err("for c in n\n  p \"{c}\"", r#"{"n": 5}"#);
        assert!(e.msg.contains("cannot iterate a number"), "got: {}", e.msg);
    }

    // ---------------------------------------------------- §9.4 ctx root

    #[test]
    fn ctx_reaches_every_scope_via_render_full() {
        let src = "for item in items\n  p \"{item} on {ctx.theme}\"";
        let out = render_full(
            src,
            &data(r#"{"items": ["a"], "ctx": "decoy"}"#),
            &data(r#"{"theme": "dark"}"#),
            Mode::Min,
        )
        .unwrap();
        assert_eq!(out.html, "<p>a on dark</p>");
    }

    // ------------------------------------------------ §11 invariants

    #[test]
    fn template_free_render_matches_compile() {
        let src = "div flex\n  p text-sm \"hi\"\n";
        assert_eq!(
            render(src, &Value::Null, Mode::Pretty).unwrap(),
            fhtml::compile(src, Mode::Pretty).unwrap()
        );
    }

    #[test]
    fn formatting_never_changes_rendered_output() {
        let src = "if  user\n    p   text-sm \"Hi, { user.name }!\"\nelse\n    p \"guest\"\nfor x,i in xs\n    li \"{ i }:{ x }\"\n";
        let formatted = fhtml::format(src).unwrap();
        let d = data(r#"{"user": {"name": "E"}, "xs": ["a", "b"]}"#);
        assert_eq!(
            render(&formatted, &d, Mode::Min).unwrap(),
            render(src, &d, Mode::Min).unwrap()
        );
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    }

    #[test]
    fn render_errors_position_like_parse_errors() {
        // columns count within the logical line's content (indent excluded),
        // matching the static parse-error convention
        let e = render_err("div\n  p \"{1 + true}\"", "{}");
        assert_eq!((e.line, e.col), (2, 4)); // the `{`

        assert!(e.msg.contains("`+`"), "got: {}", e.msg);
    }

    #[test]
    fn statement_bodies_render_at_statement_depth_in_pretty() {
        let src = "div\n  if x\n    p \"a\"\n";
        assert_eq!(
            render(src, &data(r#"{"x": true}"#), Mode::Pretty).unwrap(),
            "<div>\n  <p>a</p>\n</div>\n"
        );
    }

    // ---------------------------------------- §10.3 def (definition is inert)

    #[test]
    fn def_emits_nothing_at_definition_site() {
        // §10.3: a definition renders nothing where it stands; a file whose
        // defs are never called renders exactly its body.
        let src = "def card(title)\n  h3 \"{title}\"\np \"after\"\n";
        assert_eq!(
            render(src, &Value::Null, Mode::Min).unwrap(),
            "<p>after</p>"
        );
    }

    // --------------------------------- §10.3–§10.4 component calls render

    #[test]
    fn golden_call_from_spec_10_4() {
        let src = r#"def card(title compact=false)
  . rounded-xl p-6 {compact ? 'text-sm' : 'text-base'}
    h3 font-semibold "{title}"
    children
+card(title="Monthly stats" compact)
  p text-sm "Revenue is up 12%."
"#;
        assert_eq!(
            render(src, &Value::Null, Mode::Min).unwrap(),
            "<div class=\"rounded-xl p-6 text-sm\"><h3 class=\"font-semibold\">Monthly stats</h3>\
             <p class=\"text-sm\">Revenue is up 12%.</p></div>"
        );
    }

    #[test]
    fn defaults_evaluate_per_call_and_args_override() {
        let src = "def badge(kind='info' n=3)\n  span \"{kind}:{n}\"\n+badge\n+badge(kind='warn' n={1 + 1})\n";
        assert_eq!(
            render(src, &Value::Null, Mode::Min).unwrap(),
            "<span>info:3</span><span>warn:2</span>"
        );
    }

    #[test]
    fn argument_values_are_typed_not_strings() {
        // §10.4: unquoted args go through the expression grammar — `n=3` is
        // the number 3 (arithmetic works), bare `on` is boolean true.
        let src = "def c(n on=false)\n  p \"{n + 1}\"\n  if on\n    p \"on\"\n+c(n=3 on)\n";
        assert_eq!(
            render(src, &Value::Null, Mode::Min).unwrap(),
            "<p>4</p><p>on</p>"
        );
    }

    #[test]
    fn expression_args_pass_structured_values() {
        let src = "def who(user)\n  p \"{user.name} <{user.email}>\"\n+who(user=member.profile)\n";
        assert_eq!(
            min_with(
                src,
                r#"{"member": {"profile": {"name": "Erin", "email": "e@x.io"}}}"#
            ),
            "<p>Erin &lt;e@x.io&gt;</p>"
        );
    }

    #[test]
    fn quoted_args_interpolate_in_caller_scope() {
        let src = "def h(title)\n  h1 \"{title}\"\n+h(title=\"Hi, {user.name}!\")\n";
        assert_eq!(
            min_with(src, r#"{"user": {"name": "E"}}"#),
            "<h1>Hi, E!</h1>"
        );
    }

    #[test]
    fn component_scope_is_closed() {
        // §10.3: only the parameters are in scope in the body — the data
        // root is not visible, and a param shadows a data name for the body
        // while the caller block still sees the caller's scope.
        let src =
            "def c(x)\n  p \"{x}|{secret}\"\n  children\n+c(x='param')\n  p \"{secret}|{x}\"\n";
        assert_eq!(
            min_with(src, r#"{"secret": "s", "x": "data-x"}"#),
            "<p>param|</p><p>s|data-x</p>"
        );
    }

    #[test]
    fn ctx_reaches_component_bodies_and_defaults() {
        // §9.4/§10.3: `ctx` is in every scope; defaults may reference it.
        let src = "def c(limit={ctx.pageSize - 1})\n  p \"{ctx.site}:{limit}\"\n+c\n";
        let out = render_full(
            src,
            &Value::Null,
            &data(r#"{"site": "acme", "pageSize": 10}"#),
            Mode::Min,
        )
        .unwrap();
        assert_eq!(out.html, "<p>acme:9</p>");
    }

    #[test]
    fn children_repeats_and_empty_block_emits_nothing() {
        let src = "def twice()\n  children\n  children\n+twice\n  p \"x\"\n+twice\n";
        assert_eq!(
            render(src, &Value::Null, Mode::Min).unwrap(),
            "<p>x</p><p>x</p>"
        );
    }

    #[test]
    fn children_nests_through_component_layers() {
        // A block passed to an inner call may itself say `children` — that is
        // the *outer* component's children, rendered in its caller's scope.
        let src = "def outer()\n  +inner\n    children\ndef inner()\n  . inner\n    children\n+outer\n  p \"{msg}\"\n";
        assert_eq!(
            min_with(src, r#"{"msg": "hi"}"#),
            "<div class=\"inner\"><p>hi</p></div>"
        );
    }

    #[test]
    fn loop_variables_flow_into_args_and_blocks() {
        let src = "def row(o)\n  li \"{o.id}\"\n  children\nul\n  for o in orders\n    +row(o=o)\n      em \"{o.note}\"\n";
        assert_eq!(
            min_with(
                src,
                r#"{"orders": [{"id": 1, "note": "a"}, {"id": 2, "note": "b"}]}"#
            ),
            "<ul><li>1</li><em>a</em><li>2</li><em>b</em></ul>"
        );
    }

    #[test]
    fn recursion_renders_trees() {
        let src = "def tree(n)\n  li \"{n.v}\"\n  if n.kids\n    ul\n      for k in n.kids\n        +tree(n=k)\nul\n  +tree(n=root)\n";
        assert_eq!(
            min_with(
                src,
                r#"{"root": {"v": "a", "kids": [{"v": "b"}, {"v": "c", "kids": [{"v": "d"}]}]}}"#
            ),
            "<ul><li>a</li><ul><li>b</li><li>c</li><ul><li>d</li></ul></ul></ul>"
        );
    }

    #[test]
    fn call_depth_cap_errors_at_the_call_site() {
        // §10.3: cap of 64, error position = the exceeding call.
        let src = "def loop()\n  +loop\n+loop\n";
        let e = render(src, &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("64"), "got: {}", e.msg);
        assert_eq!((e.line, e.col), (2, 1));
    }

    #[test]
    fn call_expands_at_call_depth_in_pretty() {
        let src = "def item(t)\n  li \"{t}\"\nul\n  +item(t='a')\n";
        assert_eq!(
            render(src, &Value::Null, Mode::Pretty).unwrap(),
            "<ul>\n  <li>a</li>\n</ul>\n"
        );
    }

    #[test]
    fn error_unknown_component() {
        let e = render("+ghost\n", &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("unknown component"), "got: {}", e.msg);
        assert!(e.msg.contains("`def ghost(…)`"), "got: {}", e.msg);
    }

    #[test]
    fn error_unknown_argument_lists_params() {
        let src = "def card(title compact=false)\n  p \"{title}\"\n+card(text='x')\n";
        let e = render(src, &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("unknown argument `text`"), "got: {}", e.msg);
        assert!(e.msg.contains("`title`, `compact`"), "got: {}", e.msg);
        assert_eq!((e.line, e.col), (3, 7)); // the argument name
    }

    #[test]
    fn error_missing_required_argument() {
        let src = "def card(title compact=false)\n  p \"{title}\"\n+card(compact)\n";
        let e = render(src, &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("missing argument `title`"), "got: {}", e.msg);
        assert_eq!(e.line, 3);
    }

    #[test]
    fn error_block_to_childless_component() {
        // §10.4: silently dropping caller markup would hide real mistakes.
        let src = "def icon()\n  span \"*\"\n+icon\n  p \"lost\"\n";
        let e = render(src, &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("never uses `children`"), "got: {}", e.msg);
        assert_eq!(e.line, 3);
    }

    #[test]
    fn golden_blog_cards_def_matches_plain_fhtml() {
        // The measurement demo (−34% tokens vs plain fhtml): one `def post(…)` + three calls must
        // produce byte-identical output to the fully expanded file.
        let plain = include_str!("corpus/blog-cards.fhtml");
        let def = include_str!("corpus/blog-cards-def.fhtml");
        for mode in [Mode::Min, Mode::Pretty] {
            assert_eq!(
                render(def, &Value::Null, mode).unwrap(),
                render(plain, &Value::Null, mode).unwrap(),
                "mode {mode:?}"
            );
        }
    }

    #[test]
    fn calls_check_before_rendering_even_on_dead_branches() {
        // The component table is validated up front — a bad call inside a
        // branch this render never takes still errors.
        let src = "def c(a)\n  p \"{a}\"\nif false\n  +c(wrong='x')\n";
        let e = render(src, &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("unknown argument"), "got: {}", e.msg);
    }
}

// ------------------------------------------------------------ §10.5 include
mod includes {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use fhtml::{compile, render, render_full, render_full_from, Error, Mode, Output, Value};

    static N: AtomicUsize = AtomicUsize::new(0);

    /// A throwaway directory of fixture files, removed on drop.
    struct Fixture {
        dir: PathBuf,
    }

    impl Fixture {
        fn new(files: &[(&str, &str)]) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "fhtml-include-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            for (name, content) in files {
                let p = dir.join(name);
                fs::create_dir_all(p.parent().unwrap()).unwrap();
                fs::write(p, content).unwrap();
            }
            Fixture { dir }
        }

        fn render(&self, root: &str, mode: Mode) -> Result<Output, Error> {
            let path = self.dir.join(root);
            let src = fs::read_to_string(&path).unwrap();
            render_full_from(&src, Some(&path), &Value::Null, &Value::Null, mode)
        }

        fn html(&self, root: &str) -> String {
            self.render(root, Mode::Min).unwrap().html
        }

        fn err(&self, root: &str) -> Error {
            self.render(root, Mode::Min).unwrap_err()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn markup_splices_at_the_include_site() {
        let f = Fixture::new(&[
            ("main.fhtml", "p \"before\"\ninclude ./part\np \"after\"\n"),
            ("part.fhtml", "p \"mid\"\n"),
        ]);
        assert_eq!(f.html("main.fhtml"), "<p>before</p><p>mid</p><p>after</p>");
    }

    #[test]
    fn defs_join_one_namespace_in_both_directions() {
        // Root markup calls an included def; included markup calls a root def.
        let f = Fixture::new(&[
            (
                "main.fhtml",
                "def local(x)\n  em \"{x}\"\ninclude ./lib\n+card(title=\"T\")\n  p \"body\"\n",
            ),
            (
                "lib.fhtml",
                "def card(title)\n  h3 \"{title}\"\n  children\n+local(x=\"from lib\")\n",
            ),
        ]);
        assert_eq!(
            f.html("main.fhtml"),
            "<em>from lib</em><h3>T</h3><p>body</p>"
        );
    }

    #[test]
    fn fhtml_extension_appended_iff_absent() {
        let f = Fixture::new(&[
            ("bare.fhtml", "include ./part\n"),
            ("explicit.fhtml", "include ./part.fhtml\n"),
            ("part.fhtml", "p \"here\"\n"),
        ]);
        assert_eq!(f.html("bare.fhtml"), "<p>here</p>");
        assert_eq!(f.html("explicit.fhtml"), "<p>here</p>");
    }

    #[test]
    fn nested_include_paths_are_relative_to_each_file() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./sub/a\n"),
            ("sub/a.fhtml", "include ./b\np \"a\"\n"),
            ("sub/b.fhtml", "p \"b\"\n"),
        ]);
        assert_eq!(f.html("main.fhtml"), "<p>b</p><p>a</p>");
    }

    #[test]
    fn same_markup_file_may_be_included_twice() {
        // Literal splice semantics: markup emits at every include site.
        // (A def-carrying file included twice collides instead — below.)
        let f = Fixture::new(&[
            ("main.fhtml", "include ./part\ninclude ./part\n"),
            ("part.fhtml", "p \"x\"\n"),
        ]);
        assert_eq!(f.html("main.fhtml"), "<p>x</p><p>x</p>");
    }

    #[test]
    fn include_cycle_errors_listing_the_chain() {
        let f = Fixture::new(&[("a.fhtml", "include ./b\n"), ("b.fhtml", "include ./a\n")]);
        let e = f.err("a.fhtml");
        assert!(e.msg.contains("include cycle"), "got: {}", e.msg);
        // The chain names both files and closes on the repeated one.
        let chain = e.msg.split("include cycle:").nth(1).unwrap();
        assert!(chain.contains("b.fhtml"), "chain missing b: {}", e.msg);
        assert_eq!(
            chain.matches("a.fhtml").count(),
            2,
            "chain should open and close on a.fhtml: {}",
            e.msg
        );
    }

    #[test]
    fn self_include_is_a_cycle() {
        let f = Fixture::new(&[("a.fhtml", "include ./a\n")]);
        let e = f.err("a.fhtml");
        assert!(e.msg.contains("include cycle"), "got: {}", e.msg);
        assert_eq!((e.line, e.col), (1, 1));
    }

    #[test]
    fn def_collision_across_includes_is_an_error() {
        let f = Fixture::new(&[
            ("main.fhtml", "def card(t)\n  p \"{t}\"\ninclude ./lib\n"),
            ("lib.fhtml", "def card(t)\n  em \"{t}\"\n"),
        ]);
        let e = f.err("main.fhtml");
        assert_eq!((e.line, e.col), (3, 1));
        assert!(
            e.msg.contains("component `card`") && e.msg.contains("already defined"),
            "got: {}",
            e.msg
        );
    }

    #[test]
    fn missing_file_errors_at_the_include_line() {
        let f = Fixture::new(&[("main.fhtml", "p \"x\"\ninclude ./nope\n")]);
        let e = f.err("main.fhtml");
        assert_eq!((e.line, e.col), (2, 1));
        assert!(
            e.msg.contains("cannot include") && e.msg.contains("nope.fhtml"),
            "got: {}",
            e.msg
        );
    }

    #[test]
    fn parse_error_in_included_file_names_that_file() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./bad\n"),
            ("bad.fhtml", "p \"unclosed\n"),
        ]);
        let e = f.err("main.fhtml");
        assert_eq!((e.line, e.col), (1, 1));
        assert!(
            e.msg.contains("bad.fhtml") && e.msg.contains("1:"),
            "expected inner path and position, got: {}",
            e.msg
        );
    }

    #[test]
    fn render_errors_in_included_content_point_at_the_include_site() {
        // Positions inside included content are remapped to the include
        // line (SPEC §10.5) — coarse but always a real location in the
        // root file, and identical in the compiled JS module.
        let f = Fixture::new(&[
            ("main.fhtml", "p \"x\"\ninclude ./part\n"),
            ("part.fhtml", "p \"{1 / 0}\"\n"),
        ]);
        let e = f.err("main.fhtml");
        assert_eq!((e.line, e.col), (2, 1));
        assert!(e.msg.contains("division by zero"), "got: {}", e.msg);
    }

    #[test]
    fn errors_in_included_def_bodies_point_at_the_include_site_too() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./lib\n+boom\n"),
            ("lib.fhtml", "def boom\n  p \"{1 / 0}\"\n"),
        ]);
        let e = f.err("main.fhtml");
        assert_eq!((e.line, e.col), (1, 1), "got: {e}");
    }

    #[test]
    fn warnings_from_included_files_carry_the_path() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./part\n"),
            ("part.fhtml", "div\n  p \"a\"\n   p \"b\"\n"),
        ]);
        let out = f.render("main.fhtml", Mode::Min).unwrap();
        assert_eq!(out.warnings.len(), 1, "warnings: {:?}", out.warnings);
        assert!(
            out.warnings[0].contains("part.fhtml"),
            "got: {}",
            out.warnings[0]
        );
    }

    #[test]
    fn include_is_top_level_only() {
        for src in [
            "main\n  include ./x\n",
            "if true\n  include ./x\n",
            "def c\n  include ./x\n",
        ] {
            let e = render(src, &Value::Null, Mode::Min).unwrap_err();
            assert!(
                e.msg.contains("only at top level") && e.msg.contains("§10.5"),
                "for {src:?} got: {}",
                e.msg
            );
        }
    }

    #[test]
    fn include_needs_a_path() {
        let e = render("include\n", &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("needs a path"), "got: {}", e.msg);
        let e = render("include   \n", &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("needs a path"), "got: {}", e.msg);
    }

    #[test]
    fn include_takes_no_block() {
        let e = render("include ./x\n  p \"y\"\n", &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("cannot have children"), "got: {}", e.msg);
    }

    #[test]
    fn stdin_mode_has_no_base_path() {
        // The string-only entry points (`render`, `render_full`) have no
        // file context — a clear error, not a guessed working directory.
        let e = render_full(
            "include ./partials/head\n",
            &Value::Null,
            &Value::Null,
            Mode::Min,
        )
        .unwrap_err();
        assert_eq!((e.line, e.col), (1, 1));
        assert!(e.msg.contains("no file path"), "got: {}", e.msg);
    }

    #[test]
    fn static_compile_rejects_include_as_template_construct() {
        let e = compile("include ./x\n", Mode::Min).unwrap_err();
        assert!(
            e.msg.contains("`include`") && e.msg.contains("template construct"),
            "got: {}",
            e.msg
        );
    }

    #[test]
    fn no_templates_rejects_include_with_p0_wording() {
        let e = fhtml::compile_opts(
            "include ./x\n",
            &fhtml::Options {
                mode: Mode::Min,
                templates: false,
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(e.msg.contains("--no-templates"), "got: {}", e.msg);
    }

    #[test]
    fn fmt_reprints_include_lines_in_place() {
        // fmt never touches the filesystem: the line reprints as written,
        // in position, and formatting is idempotent.
        let src = "// head partial\ninclude ./partials/head\ndef c\n  p \"x\"\ninclude ./partials/foot\np   \"tail\"\n";
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(
            formatted,
            "// head partial\ninclude ./partials/head\ndef c\n  p \"x\"\ninclude ./partials/foot\np \"tail\"\n"
        );
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
    }

    // ------------------------------------------ #!shorthand scope (SPEC §3.2)

    #[test]
    fn shorthand_directive_does_not_leak_into_included_files() {
        let f = Fixture::new(&[
            ("main.fhtml", "#!shorthand\ndiv fx\ninclude ./part\n"),
            ("part.fhtml", "p ti4\n"),
        ]);
        assert_eq!(
            f.html("main.fhtml"),
            "<div class=\"flex\"></div><p class=\"ti4\"></p>"
        );
    }

    #[test]
    fn included_directive_does_not_leak_into_the_includer() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./part\np ti4\n"),
            ("part.fhtml", "#!shorthand\nspan fx\n"),
        ]);
        assert_eq!(
            f.html("main.fhtml"),
            "<span class=\"flex\"></span><p class=\"ti4\"></p>"
        );
    }

    #[test]
    fn forced_policy_reaches_included_files() {
        use fhtml::{render_opts_from, Options, ShorthandPolicy};
        let f = Fixture::new(&[
            ("on.fhtml", "include ./plain\n"),
            ("plain.fhtml", "p ti4\n"),
            ("off.fhtml", "include ./short\n"),
            ("short.fhtml", "#!shorthand\np ti4\n"),
        ]);
        let html = |root: &str, shorthand| {
            let path = f.dir.join(root);
            let src = fs::read_to_string(&path).unwrap();
            render_opts_from(
                &src,
                Some(&path),
                &Value::Null,
                &Value::Null,
                &Options {
                    shorthand,
                    ..Default::default()
                },
            )
            .unwrap()
            .html
        };
        // Force-on decodes in an included file that never opted in…
        assert_eq!(
            html("on.fhtml", ShorthandPolicy::On),
            "<p class=\"text-indigo-400\"></p>"
        );
        // …and force-off suppresses an included file's own directive.
        assert_eq!(
            html("off.fhtml", ShorthandPolicy::Off),
            "<p class=\"ti4\"></p>"
        );
    }

    // ---- `deps_from` / `fhtml deps` — the include graph as a watch list
    // (graph semantics are SPEC §10.5).

    impl Fixture {
        fn deps(&self, root: &str) -> Result<Vec<PathBuf>, Error> {
            let path = self.dir.join(root);
            let src = fs::read_to_string(&path).unwrap();
            fhtml::deps_from(&src, Some(&path))
        }

        /// Canonicalized fixture path — what `deps` output must equal
        /// (`temp_dir` is itself a symlink on macOS: /var → /private/var).
        fn canon(&self, rel: &str) -> PathBuf {
            fs::canonicalize(self.dir.join(rel)).unwrap()
        }
    }

    #[test]
    fn deps_lists_nested_includes_in_first_include_order() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./sub/a\ninclude ./top\n"),
            ("sub/a.fhtml", "include ./b\np \"a\"\n"),
            ("sub/b.fhtml", "p \"b\"\n"),
            ("top.fhtml", "p \"t\"\n"),
        ]);
        // Pre-order: an includer precedes its own includes; `top` comes last.
        assert_eq!(
            f.deps("main.fhtml").unwrap(),
            vec![
                f.canon("sub/a.fhtml"),
                f.canon("sub/b.fhtml"),
                f.canon("top.fhtml")
            ]
        );
    }

    #[test]
    fn deps_of_an_include_free_file_is_empty() {
        let f = Fixture::new(&[("main.fhtml", "p \"solo\"\n")]);
        assert_eq!(f.deps("main.fhtml").unwrap(), Vec::<PathBuf>::new());
    }

    #[test]
    fn deps_deduplicates_a_twice_included_file() {
        let f = Fixture::new(&[
            ("main.fhtml", "include ./part\ninclude ./part\n"),
            ("part.fhtml", "p \"x\"\n"),
        ]);
        assert_eq!(f.deps("main.fhtml").unwrap(), vec![f.canon("part.fhtml")]);
    }

    #[test]
    fn deps_errors_are_the_compile_errors() {
        // Cycle and missing-target failures must report exactly as a compile
        // of the same root does — the plugin surfaces them verbatim.
        let cycle = Fixture::new(&[("a.fhtml", "include ./b\n"), ("b.fhtml", "include ./a\n")]);
        let d = cycle.deps("a.fhtml").unwrap_err();
        let c = cycle.err("a.fhtml");
        assert_eq!((d.line, d.col, &d.msg), (c.line, c.col, &c.msg));
        assert!(d.msg.contains("include cycle"), "got: {}", d.msg);

        let missing = Fixture::new(&[("main.fhtml", "include ./nope\n")]);
        let d = missing.deps("main.fhtml").unwrap_err();
        let c = missing.err("main.fhtml");
        assert_eq!((d.line, d.col, &d.msg), (c.line, c.col, &c.msg));
    }

    #[test]
    fn deps_cli_prints_absolute_paths_one_per_line() {
        use std::process::Command;
        let f = Fixture::new(&[
            ("main.fhtml", "include ./sub/a\n"),
            ("sub/a.fhtml", "include ./b\n"),
            ("sub/b.fhtml", "p \"b\"\n"),
            ("plain.fhtml", "p \"solo\"\n"),
        ]);
        let run = |root: &str| {
            Command::new(env!("CARGO_BIN_EXE_fhtml"))
                .args(["deps", f.dir.join(root).to_str().unwrap()])
                .output()
                .unwrap()
        };
        let out = run("main.fhtml");
        assert!(out.status.success(), "deps must succeed");
        let stdout = String::from_utf8(out.stdout).unwrap();
        let lines: Vec<&str> = stdout.lines().collect();
        assert_eq!(
            lines,
            vec![
                f.canon("sub/a.fhtml").to_str().unwrap().to_string(),
                f.canon("sub/b.fhtml").to_str().unwrap().to_string(),
            ]
        );
        assert!(lines.iter().all(|l| PathBuf::from(l).is_absolute()));

        // No includes → empty stdout, still success.
        let plain = run("plain.fhtml");
        assert!(plain.status.success());
        assert!(plain.stdout.is_empty(), "no includes prints nothing");
    }
}

// ---------------------------------------------------- #!shorthand (SPEC §3.2)

mod shorthand_directive {
    use fhtml::{compile, compile_opts, compile_to_js, Mode, Options, ShorthandPolicy};

    /// Every placement violation quotes this exact message (SPEC §3.2).
    const PLACEMENT: &str = "`#!shorthand` must be the first non-blank line and start at column 1";

    fn min(src: &str) -> String {
        compile(src, Mode::Min).unwrap()
    }

    fn error(src: &str) -> String {
        compile(src, Mode::Min).unwrap_err().to_string()
    }

    fn with_policy(src: &str, shorthand: ShorthandPolicy) -> Result<String, String> {
        compile_opts(
            src,
            &Options {
                mode: Mode::Min,
                shorthand,
                ..Default::default()
            },
        )
        .map(|o| o.html)
        .map_err(|e| e.to_string())
    }

    #[test]
    fn directive_decodes_codes_variants_and_escapes() {
        assert_eq!(
            min("#!shorthand\ndiv fx ti4 hover:bb5 =ti4 not-a-code\n"),
            "<div class=\"flex text-indigo-400 hover:bg-blue-500 ti4 not-a-code\"></div>"
        );
    }

    #[test]
    fn without_directive_tokens_stay_literal() {
        assert_eq!(min("div fx ti4\n"), "<div class=\"fx ti4\"></div>");
    }

    #[test]
    fn leading_blank_lines_before_directive_are_fine() {
        assert_eq!(
            min("\n\n#!shorthand\ndiv fx\n"),
            "<div class=\"flex\"></div>"
        );
    }

    #[test]
    fn comment_before_directive_is_rejected() {
        let e = error("// prelude\n#!shorthand\ndiv fx\n");
        assert!(e.contains(PLACEMENT), "got: {e}");
        assert!(e.starts_with("2:1"), "got: {e}");
    }

    #[test]
    fn indented_first_directive_is_rejected_at_column_1() {
        let e = error("  #!shorthand\ndiv fx\n");
        assert!(e.contains(PLACEMENT), "got: {e}");
        assert!(e.starts_with("1:1"), "got: {e}");
    }

    #[test]
    fn nested_directive_is_rejected_and_cannot_leak() {
        // The original hazard: a mid-file directive silently rewrote every
        // later class in the file. It is now a hard error.
        let e = error("div\n  #!shorthand\n  span fx\np ti4\n");
        assert!(e.contains(PLACEMENT), "got: {e}");
        assert!(e.starts_with("2:1"), "got: {e}");
    }

    #[test]
    fn duplicate_directive_is_rejected() {
        let e = error("#!shorthand\n#!shorthand\ndiv fx\n");
        assert!(e.contains(PLACEMENT), "got: {e}");
        assert!(e.starts_with("2:1"), "got: {e}");
    }

    #[test]
    fn directive_takes_no_arguments() {
        let e = error("#!shorthand on\ndiv\n");
        assert!(e.contains("takes nothing after it"), "got: {e}");
    }

    #[test]
    fn unknown_directive_is_named() {
        let e = error("#!strict\ndiv\n");
        assert!(e.contains("unknown directive `#!strict`"), "got: {e}");
    }

    #[test]
    fn force_on_expands_without_a_directive() {
        assert_eq!(
            with_policy("div fx ti4\n", ShorthandPolicy::On).unwrap(),
            "<div class=\"flex text-indigo-400\"></div>"
        );
    }

    #[test]
    fn force_on_with_directive_is_redundant_not_an_error() {
        let src = "#!shorthand\ndiv fx\n";
        assert_eq!(
            with_policy(src, ShorthandPolicy::On).unwrap(),
            with_policy(src, ShorthandPolicy::Auto).unwrap()
        );
    }

    #[test]
    fn force_off_is_lexical_codes_and_escapes_both_inert() {
        // Lexical-off (SPEC §3.2): the file parses as if no directive were
        // present, so `=` is not shorthand syntax either.
        assert_eq!(
            with_policy("#!shorthand\ndiv ti4 =ti4\n", ShorthandPolicy::Off).unwrap(),
            "<div class=\"ti4 =ti4\"></div>"
        );
    }

    #[test]
    fn force_off_still_validates_placement() {
        let e = with_policy("div\n  #!shorthand\n", ShorthandPolicy::Off).unwrap_err();
        assert!(e.contains(PLACEMENT), "got: {e}");
    }

    #[test]
    fn escape_needs_the_directive_to_mean_anything() {
        // No directive, Auto: `=ti4` is just a (weird) literal class.
        assert_eq!(min("div =ti4\n"), "<div class=\"=ti4\"></div>");
    }

    #[test]
    fn js_target_sees_expanded_classes() {
        let js = compile_to_js("#!shorthand\ndiv fx ti4\n", Mode::Min)
            .unwrap()
            .html;
        assert!(js.contains("text-indigo-400"), "module:\n{js}");
        assert!(!js.contains("ti4\""), "module leaked a code:\n{js}");
    }

    // ------------------------------------------------------------- fmt

    #[test]
    fn fmt_preserves_directive_and_authored_codes() {
        let src = "#!shorthand\ndiv fx ti4 =ti4\n";
        let formatted = fhtml::format(src).unwrap();
        assert_eq!(formatted, "#!shorthand\n. fx ti4 =ti4\n");
        // Idempotent, and compile-equivalent to the original (SPEC §11).
        assert_eq!(fhtml::format(&formatted).unwrap(), formatted);
        assert_eq!(min(&formatted), min(src));
    }

    #[test]
    fn fmt_normalizes_blank_lines_ahead_of_the_directive() {
        assert_eq!(
            fhtml::format("\n#!shorthand\ndiv fx\n").unwrap(),
            "#!shorthand\n. fx\n"
        );
    }

    #[test]
    fn fmt_leaves_directive_free_files_alone() {
        assert_eq!(fhtml::format("div fx ti4\n").unwrap(), ". fx ti4\n");
    }

    // ------------------------------------------ quoted class="…" attributes

    #[test]
    fn class_attr_tokens_decode_like_bare_tokens() {
        // Decoding applies to every class token, quoted-attr or bare
        // (SPEC §3.2) — the two forms merge into one class list, so they
        // must mean the same thing or `fmt`'s merge would change output.
        assert_eq!(
            min("#!shorthand\ndiv(class=\"fx =p4 custom\") p4\n"),
            "<div class=\"flex p4 custom p-4\"></div>"
        );
    }

    #[test]
    fn class_attr_stays_verbatim_without_directive() {
        assert_eq!(
            min("div(class=\"fx =p4\")\n"),
            "<div class=\"fx =p4\"></div>"
        );
    }

    #[test]
    fn fmt_of_a_class_attr_under_the_directive_is_output_preserving() {
        // Regression: the attr form used to stay verbatim while the bare
        // form decoded, so fmt's bare reprint changed the compiled output.
        let src = "#!shorthand\ndiv(class=\"fx\") p4\n";
        assert_eq!(min(&fhtml::format(src).unwrap()), min(src));
    }

    // -------------------------------------------- fmt --contract / --expand

    use fhtml::{format_shorthand, FmtShorthand};

    fn contract(src: &str) -> String {
        format_shorthand(src, FmtShorthand::Contract).unwrap()
    }

    fn expand(src: &str) -> String {
        format_shorthand(src, FmtShorthand::Expand).unwrap()
    }

    #[test]
    fn contract_rewrites_to_codes_and_adds_the_directive() {
        // Codes where they round-trip, `=`-escapes where the class would
        // read as something else (`p4` decodes; a leading `=` doubles),
        // verbatim otherwise.
        let src = "div flex items-center p-4 hover:bg-blue-500 custom p4 =foo\n";
        let got = contract(src);
        assert_eq!(got, "#!shorthand\n. fx ic p4 hover:bb5 custom =p4 ==foo\n");
        assert_eq!(min(&got), min(src));
        assert_eq!(contract(&got), got); // idempotent
    }

    #[test]
    fn contract_normalizes_a_file_already_in_shorthand_form() {
        // Verbatim classes in a directive file contract too; authored
        // escapes and codes are already canonical and stay put.
        assert_eq!(
            contract("#!shorthand\ndiv flex =p4 fx\n"),
            "#!shorthand\n. fx =p4 fx\n"
        );
    }

    #[test]
    fn contract_leaves_interpolation_alone() {
        assert_eq!(contract("div flex {cls}\n"), "#!shorthand\n. fx {cls}\n");
    }

    #[test]
    fn expand_decodes_and_drops_the_directive() {
        let src = "#!shorthand\ndiv fx ti4 =ti4 custom\n";
        let got = expand(src);
        assert_eq!(got, ". flex text-indigo-400 ti4 custom\n");
        assert_eq!(min(&got), min(src));
        assert_eq!(expand(&got), got); // idempotent
    }

    #[test]
    fn expand_of_a_directive_free_file_is_plain_fmt() {
        // Without the directive `fx` is a literal class — nothing to expand.
        assert_eq!(expand("div fx =ti4\n"), ". fx =ti4\n");
    }

    #[test]
    fn expand_moves_a_hostile_meaning_into_the_attr_form() {
        // `=#foo` under the directive means the literal class `#foo`, which
        // printed bare would reparse as an id — it must ride in class="…".
        let src = "#!shorthand\ndiv =#foo\n";
        let got = expand(src);
        assert_eq!(got, ".(class=\"#foo\")\n");
        assert_eq!(min(&got), min(src));
    }

    #[test]
    fn contract_then_expand_is_canonical_plain_fmt() {
        let src = "div flex items-center p-4 custom p4 =foo\n  p text-lg \"hi\"\n";
        assert_eq!(expand(&contract(src)), fhtml::format(src).unwrap());
    }
}
