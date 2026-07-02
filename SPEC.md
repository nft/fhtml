# fhtml Language Specification

Version 0.1 (draft). This document is normative.
Layers: **§1–§8 = static markup** (always available), **§9–§10 = template layer**.
A file using only §1–§8 constructs compiles to static HTML.

---

## 1. Source files

- Extension `.fhtml`, encoding UTF-8, newline `\n` (`\r\n` accepted, normalized).
- A **logical line** is a physical line, joined with any following physical lines while the
  line ends in `\` (the `\`, the newline, and the next line's leading whitespace collapse to
  a single space).
- Continuation applies **only to element, statement, and component-call lines**. Lines whose
  first token (of the initial physical line) classifies them as comments (`//`, `//!`), raw
  passthrough (`<`), or text blocks (`|`) never join — a trailing `\` there is literal
  content. Joining is therefore a per-line decision, not a raw preprocessing pass.
- Blank lines (whitespace-only) are ignored and never affect block structure.

## 2. Indentation and blocks

1. Nesting depth is expressed by leading whitespace on the logical line's first physical line.
2. The **indent unit** is fixed by the first indented line in the file: either one tab or a
   run of N spaces (N ≥ 1). Every deeper level must be exactly *parent indent + one unit*.
3. Mixing tabs and spaces in indentation is an error.
4. A line indented one unit deeper than line *L* is a child of *L*. Skipping levels
   (two units deeper at once) is an error. Dedents must return to an existing open level.

## 3. Line forms

Classified by the logical line's first token:

| First token | Form | Section |
|---|---|---|
| `//` | silent comment (not emitted) | §3.1 |
| `//!` | emitted comment → `<!-- … -->` | §3.1 |
| `<` | raw HTML passthrough | §8 |
| `\|` | text block line | §6.2 |
| `doctype` | `<!DOCTYPE html>` | §7 |
| `if` `elif` `else` `for` `empty` `def` `slot` `include` | statement (template layer) | §10 |
| `+name` | component call (template layer) | §10.4 |
| anything else | element line | §4 |

Reserved words are reserved **only in first-token position**. An element literally named
`if` etc. must use raw passthrough (§8).

### 3.1 Comments

`// text` — compiler-only, produces no output. `//! text` — emits `<!-- text -->`.
Indented lines under a comment belong to the comment (silent for `//`, included for `//!`).

## 4. Element lines

Anatomy (order is fixed; every part except the tag is optional):

```
tag(attrs) tokens… "text" > chained-element
```

### 4.1 Tag

- A name matching `[A-Za-z][A-Za-z0-9-]*` (covers HTML, SVG, custom elements).
- `.` alone as the tag means `div`.
- Unknown tags are emitted as-is (no whitelist).
- **Pug divergence (deliberate):** `.card` or `#hero` as the first token is an **error**,
  not a div shorthand — the diagnostic must suggest `. card` / `. #hero`. Supporting Pug's
  form would reintroduce `.class`-literal parsing and silently mis-split habits like
  `.flex.items-center` (one bogus class `flex.items-center`), which is the exact failure
  mode fhtml exists to eliminate.

### 4.2 The tokenizer contract (normative)

1. After the tag (and its attached attrs, §4.3), the rest of the logical line is split on
   whitespace — except inside `"…"` text and `{…}` interpolation. All delimiter counting
   (braces *and* the attrs parens of §4.3) ignores delimiters inside single- or
   double-quoted segments. Parentheses have grouping power **only** in the tag-attached
   attrs segment; inside a class token they are ordinary characters. Class tokens can never
   contain whitespace — Tailwind itself requires `_` in place of spaces inside arbitrary
   values (`bg-[url('/a_b.png')]`) — so whitespace-first splitting is always safe.
2. Each token is classified **by its leading characters only**:
   - `"` → inline text (§6.1)
   - `{` → interpolation token (§9; class position)
   - `#` → id (§4.4)
   - the standalone token `>` → inline chain (§4.6)
   - anything else → **class name, copied to output verbatim**.
3. The compiler never parses the interior of a class token. All Tailwind syntax —
   `py-2.5`, `w-1/2`, `hover:bg-zinc-200`, `active:translate-y-[0.5px]`,
   `data-[state=open]:bg-red-500`, `bg-[url(/x.png)]`, `[&>li]:mt-0`, `!mt-0`, `-mt-2`,
   `*:pt-2`, `@lg:flex` — passes through untouched.

### 4.3 Attributes

- Attrs appear in parentheses **immediately after the tag, no space**: `img(src=/a.png)`.
  A `(` anywhere else is an ordinary character inside a class token.
- The attrs segment ends at the first `)` **outside any quoted value** — parens inside
  quotes don't count: `button(onclick="alert('(hi)')")` parses correctly.
- Inside parens, entries are separated by whitespace:
  - `name` — boolean attribute → emitted as bare `name`.
  - `name=value` — no whitespace around `=`.
- Attribute **names**: any run of characters except whitespace, `=`, `)` (covers `data-*`,
  `aria-*`, `hx-*`, `@click`, `:bind`, `x-on:click` for downstream frameworks; fhtml assigns
  them no meaning).
- Attribute **values**:
  - Unquoted: runs until whitespace or `)`. May be exactly one `{expr}`.
  - Quoted with `"…"` or `'…'`: may contain whitespace and `{expr}` segments (§9.2).
- Duplicate attribute names: error, except `class` (§4.5).
- Escapes inside quoted values: `\"`, `\'`, `\\`, `\{`.

### 4.4 Id

A token `#name` sets the element id. More than one id token is an error. `name` is any run
of non-whitespace characters.

### 4.5 Classes

- All class tokens (and class-position interpolations) accumulate in source order into one
  `class` attribute.
- A `class=…` inside parens is merged first, followed by bare class tokens.

### 4.6 Inline chain

A standalone `>` token ends the current element's inline content; the remainder of the line
is parsed as a new element line, which becomes the **sole inline child**:

```fhtml
li > a(href=/docs) font-medium hover:underline "Docs"
```

Chains may repeat (`li > a > span …`). Indented children under the line attach to the
**innermost** (last) element of the chain — consistent with normal indentation, which always
nests under the deepest open element. Text before a `>` belongs to the outer element and
precedes the chained child.

A chain is the wrong tool when the *outer* element needs further children — write two lines
instead. The linter should warn when a chain places block-level children inside an inline
element (`li > a` followed by an indented `ul` produces a `<ul>` inside the `<a>`).

## 5. Content model

An element's content is, in order: its inline text (if any), its chained child (if any),
then its indented children. Each child is emitted on its own line in the output (§11) —
inter-element whitespace therefore collapses per normal HTML rules. Markup that requires
*exact* inline whitespace must use raw passthrough (§8).

## 6. Text

### 6.1 Inline text

At most one `"…"` token per element line, positioned after classes/attrs (before a `>`
chain, if both are present). Escapes: `\"`, `\\`, `\{`, `\n`. Content is HTML-escaped on
output (`& < > "`). May contain `{expr}` interpolation (§9).

### 6.2 Text blocks

A line whose first token is `|` contributes the rest of the line (one leading space after
`|` is stripped) as a text child of the parent element. Consecutive `|` lines are separate
text lines in the output (HTML collapses the newline to a space). No quote escaping needed;
`\{` escapes a literal brace; interpolation allowed.

```fhtml
p text-sm text-gray-600
  | He said "hello" and left.
  | Second line of the same paragraph.
```

## 7. Void elements & doctype

`area base br col embed hr img input link meta source track wbr` are emitted without a
closing tag; giving them children or text is an error. `doctype` and `doctype html` (the
alias absorbs the Pug/HTML habit) both emit `<!DOCTYPE html>`; any other trailing token is
an error. Because `doctype` is reserved in first-token position (§12), a malformed doctype
line can never silently parse as an element.

## 8. Raw passthrough

A line whose first character (after indentation) is `<` is emitted **verbatim**, along with
every following line indented deeper than it (dedented by the raw line's own indentation).
No escaping, no interpolation, no parsing. This is the escape hatch for inline SVG paths,
embeds, exotic whitespace, and elements whose names collide with reserved words.

---

## 9. Template layer: interpolation

### 9.1 Forms

- `{expr}` — evaluate, stringify (§9.4), **HTML-escape**, emit.
- `{!expr}` — evaluate, stringify, emit **raw**. Allowed only in content positions (inline
  text, text blocks, class position is *not* content — see below); **forbidden inside
  attribute values**. Raw output in class position is also forbidden; `{expr}` there is
  already emitted attribute-escaped.

### 9.2 Contexts

| Context | Example | Notes |
|---|---|---|
| Inline text / text block | `p "Hi, {user.name}"`, `\| total: {n}` | escaped; `{!x}` allowed |
| Quoted attr value | `title="Profile of {user.name}"` | escaped; `{!x}` forbidden |
| Unquoted attr value | `href={user.url}` | must be the entire value |
| Class position | `{active ? 'bg-blue-600' : 'bg-gray-100'}` | whole token starts with `{`; result splits on whitespace into class names |

Literal `{` in text or quoted values is written `\{`. In static-only files (no template layer),
`{` has no meaning and needs no escape; the compiler flag `--no-templates` enforces static-only.

### 9.3 Expression grammar

```
expr     = ternary
ternary  = or ("?" expr ":" expr)?
or       = and ("||" and)*
and      = equality ("&&" equality)*
equality = compare (("==" | "!=") compare)*
compare  = additive (("<" | "<=" | ">" | ">=") additive)*
additive = mult (("+" | "-") mult)*
mult     = unary (("*" | "/" | "%") unary)*
unary    = ("!" | "-") unary | postfix
postfix  = primary ("." name | "[" expr "]")*
primary  = number | string | "true" | "false" | "null"
         | name | "(" expr ")"
string   = "'…'" | '"…"'        ; prefer single quotes inside markup
number   = decimal integer or float
name     = [A-Za-z_][A-Za-z0-9_]*
```

This grammar is **closed**: no function calls, no lambdas, no assignments, no host-language
escape. It is identical across all compiler backends.

### 9.4 Evaluation semantics

- Data model: null, boolean, number, string, list, map (whatever the host passes in).
- Resolving a missing path/key/index yields `null` (never an error).
- The reserved root name **`ctx`** resolves in *every* scope — including component bodies —
  to a read-only, host-provided context map (current user, theme, i18n strings: data that
  would otherwise be prop-drilled through every component). `ctx` cannot be shadowed by
  parameters or loop variables.
- **Falsy**: `null`, `false`, `0`, `""`, empty list, empty map. Everything else truthy.
- `==` is deep structural equality. `+` adds two numbers; if either operand is a string, the
  other is stringified (rules below) and concatenated; lists/maps in `+` are an error.
  Interpolation (`"{n} items"`) remains the idiomatic form — `+`-coercion exists so the
  occasional `{'#' + id}` doesn't error. Other arithmetic/comparison requires numbers.
- Stringification: `null` → empty string; booleans → `true`/`false`; numbers in shortest
  round-trip form; lists/maps in interpolation are an error (catches mistakes early).

## 10. Template layer: statements

### 10.1 `if` / `elif` / `else`

```fhtml
if user
  p "Welcome back, {user.name}"
elif invited
  p "Finish signing up"
else
  a(href=/login) "Sign in"
```

`elif`/`else` must appear at the same indent as their `if`, with no other siblings between.

### 10.2 `for` / `empty`

```
for name in expr
for name, index in expr
```

Iterates lists (index = position) and maps (name = value, index = key). The optional
`empty` block (same indent, directly after) renders when the iterable is empty or `null`.
Iterating anything else — a number, boolean, or string — is a render error (strings are not
character sequences in fhtml). Loop variables shadow outer names within the block.

### 10.3 `def` and `slot`

```
def name(param param=default …)
```

- Defines a component; emits nothing at definition site. Component names share a namespace
  per file (plus includes); redefinition is an error.
- **Defaults are expressions** (§9.3), not attribute-value strings: in
  `def alert(kind='info' compact=false max=3)`, `compact` is boolean and `max` is a number —
  never the strings `"false"`/`"3"`. An unquoted default must contain no whitespace; brace a
  spaced expression: `limit={ctx.pageSize - 1}`. Defaults may reference `ctx` but not other
  parameters, and are evaluated at each call.
- Inside the body, only the parameters and `slot` are in scope — components are closed over
  nothing (explicit data flow, no surprise coupling).
- `slot` (statement, alone on its line) emits the caller's block. Multiple `slot`s repeat
  it. v0.1 has **default slot only** (named slots: open question).

### 10.4 Component call

```fhtml
+card(title="Monthly stats" compact)
  p text-sm "Revenue is up 12%."
```

- `+name(args)` — arguments use the attribute *shape* (§4.3) but expression *values*: bare
  `name` = `true`; quoted values are strings (with `{}` interpolation); **unquoted values
  are parsed with the expression grammar** (§9.3) — `count=3`, `show=false`,
  `user=member.profile` pass a number, a boolean, a value — never coerced strings. An
  unquoted argument must contain no whitespace; brace anything spaced: `n={a + b}`.
- Arguments are **named-only**; unknown names are errors; parameters without defaults are
  required.
- The indented block becomes the slot content, evaluated in the **caller's** scope.

### 10.5 `include`

```
include ./partials/head
```

- Path relative to the including file; `.fhtml` appended if absent. The included file's
  `def`s become available; its top-level markup is emitted at the include site.
- Include cycles are an error. `include` is allowed only at top level of a file.

---

## 11. Compilation semantics

- **Attribute order** (deterministic): id (from `#id`), then paren attrs in source order,
  then merged `class`.
- **Escaping**: text and attribute values are entity-escaped (`& < > "`); class names and
  raw passthrough are emitted byte-for-byte; `{!expr}` output is unescaped by definition.
- **Output modes**: `--pretty` (2-space indented, default for `build`) and `--min`
  (no inter-tag whitespace, default for pipelines/stdout). Both modes must produce the same
  **element tree**; inter-element whitespace text nodes are *not* part of the contract and
  will differ (in HTML, whitespace between inline elements is rendering-significant —
  markup that depends on it must use raw passthrough, §5/§8).
- **Targets**: static HTML (static path, chosen automatically when a file uses no template
  constructs); `--target=js` emits an ES module exporting `(data) => string`.
- **Errors** carry file, line, column, and the offending token; parsing is strict — there
  is no recovery mode that silently guesses (an agent retry loop needs precise, honest
  errors more than it needs leniency).

## 12. Reserved words

First-token position only: `doctype if elif else for empty def slot include`.
In expressions: `true false null ctx`.
Sigils with fixed meaning at token start: `" { # > \| // //! < \ + .` (the last three only
in first-token position). Everything else is a tag name or a class name.