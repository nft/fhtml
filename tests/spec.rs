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
fn error_template_layer_keyword() {
    let e = error("if user\n  p \"hi\"");
    assert!(e.contains("template layer"), "got: {e}");
}

#[test]
fn error_template_interpolation_class_position() {
    let e = error("div {active}");
    assert!(e.contains("template layer"), "got: {e}");
}

#[test]
fn error_component_call() {
    let e = error("+card(title=\"x\")");
    assert!(e.contains("template layer"), "got: {e}");
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
