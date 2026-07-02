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
fn error_indent_skips_level() {
    let e = error("div\n    p \"deep\"\n  p \"ok\"");
    assert!(e.contains("indentation"), "got: {e}");
}

#[test]
fn error_mixed_tabs_and_spaces() {
    let e = error("div\n\tp \"a\"\n  p \"b\"");
    assert!(e.contains("tabs and spaces"), "got: {e}");
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
