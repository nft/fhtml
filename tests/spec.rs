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
fn error_include_is_not_implemented() {
    let e = error("include ./partials/head");
    assert!(e.contains("composition layer"), "got: {e}");
    assert!(e.contains("10.5"), "got: {e}");
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

    // TEMPORARY: calls parse end-to-end
    // but rendering them is not implemented yet — this stub error flips there.
    #[test]
    fn call_render_is_stage_2_stub() {
        let src = "def card(title)\n  h3 \"{title}\"\n+card(title='x')\n";
        let e = render(src, &Value::Null, Mode::Min).unwrap_err();
        assert!(e.msg.contains("not implemented"), "got: {}", e.msg);
        assert_eq!(e.line, 3);
    }
}
