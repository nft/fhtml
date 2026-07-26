---
name: fhtml
description: Write FHTML (.fhtml) markup — an indentation-based language compiling 1:1 to HTML. Use when creating or editing .fhtml files, factoring components with def/include, templating with {expr}, or running the fhtml CLI.
---

# FHTML

fhtml is a whitespace-based markup language that compiles 1:1 to HTML: no
closing tags, no angle brackets, bare tokens after a tag are the class list.
Files end in `.fhtml`.

**Before writing any fhtml, read `references/language.md`** — the complete
syntax reference (line shape, attributes, text, templates, components). This
file covers what that one doesn't: how to structure a project, when to factor
components, and the workflow that keeps output correct and readable.

The golden loop: **write → `fhtml fmt` → build**. Never hand-finish a file
without compiling it.

## Workflow

```sh
fhtml fmt page.fhtml                  # canonical formatting, in place
fhtml page.fhtml                      # compile to stdout (minified) — the correctness check
fhtml page.fhtml --pretty -o out.html # readable output to a file
fhtml build src/ -o dist/             # compile a directory tree
fhtml page.fhtml --data data.json     # render the template layer
fhtml page.fhtml --deny-warnings      # CI: any warning fails the build
```

- Compile errors carry `line:col` — fix and re-run until clean.
- Run `fhtml fmt` after every edit; the formatter never changes compiled
  output, so there is no reason to skip it.
- Treat warnings as errors while developing (`--deny-warnings`) — a warning
  today is a corrupted class list tomorrow.
- Template data goes in JSON files (`--data`, plus read-only `--ctx`), never
  hardcoded into the markup when the task calls for templating.

## Project structure

Keep pages and shared markup in separate files:

```
src/
  index.fhtml          # one page = one .fhtml file
  about.fhtml
  partials/
    layout.fhtml       # shared head/nav/footer defs
    card.fhtml         # one component family per file
static/
  js/menu.js           # behavior lives in .js files, not in markup
```

- **One page per `.fhtml` file.** A page that grows past ~150 lines should be
  split: move self-contained sections into partials and `include` them.
- **Shared `def`s live in partial files** under `partials/` (or `_inc/`),
  spliced with `include ./partials/card` — an include's `def`s join the
  namespace and its markup (if any) emits at the include site. Keep def-only
  partials (no top-level markup) for component libraries.
- **One component family per partial file** — `card.fhtml` holds `card` and
  its variants, not every component in the project.
- Include paths are relative to the including file; cycles and duplicate
  `def` names are errors.

## Components: when and how

- **Factor with `def` only when markup repeats.** Two or more instances of
  the same shape with only text/attribute values changing → one `def`,
  instantiated with `+name(…)`. Single-use markup stays plain — wrapping it
  in a def adds noise and tokens for nothing.
- **Parameterize every difference between the repeats** — ids, `aria-label`s,
  a `checked` flag, the highlighted tier's extra classes. Compare instances
  token by token; a difference you flatten away corrupts the output. If the
  blocks differ in *structure* (not just values), leave them plain.
- **Longest varying content goes in the `children` block**, not a parameter:
  sentences and paragraphs read better as an indented `|` block under the
  `+call` than as a giant string argument.
- **Names use underscores, never hyphens** — `def blog_post(img_src)`, not
  `def blog-post(img-src)` (`-` is minus).
- **The one quoting trap:** in a `+call`, an unquoted value is an
  *expression* — `n=3` is a number, `wide=false` a boolean, but `href=/fast`
  is an ERROR. **Every string argument must be double-quoted, including
  URLs**: `+card(href="/fast")`, even though `a(href=/fast)` is fine on a
  plain tag. Same rule for `def` parameter defaults.
- A def body sees only its parameters — it closes over nothing.
- Don't leave dead defs: every `def` you write should be called at least once.

## Classes and Tailwind

- Emit **plain Tailwind classes only**. Never emit `#!shorthand` codes — the
  shorthand codebook is write-time storage compression (`fhtml fmt
  --contract`), not an output format.
- **Never build class names from expressions** — Tailwind's scanner is
  static. `bg-{color}-100` is a compile ERROR; `{"bg-" + color}` compiles but
  warns and produces classes Tailwind will never see. Interpolate whole class
  names: `button {active ? "bg-blue-600 text-white" : "bg-gray-100"}`.
- Conditional classes need no helper — in class position, falsy results emit
  nothing (the clsx rule): `{active && 'bg-indigo-600 text-white'}` adds the
  classes or nothing; `{size || 'text-sm'}` supplies a default.
- Negation needs a space after the brace — `{ !done && 'opacity-50'}` —
  because `{!` means raw interpolation.

## Scripts and styles

`script` and `style` bodies are raw text: every line indented under the tag
emits verbatim (no `|` prefix), with no escaping and **no interpolation**.
Blank lines and relative indentation inside the body are preserved. (The
pre-0.4 `|`-line form still parses; `fhtml fmt` migrates it.)

- **Do not write inline JavaScript blocks.** Put behavior in a separate `.js`
  file and reference it: `script(src=/js/menu.js defer)`. Inline `script`
  bodies can't use template data, bloat every page they're pasted into, and
  are where markup files rot.
- Same for CSS: nontrivial styles belong in a stylesheet loaded with
  `link(rel=stylesheet href=/site.css)` (usually Tailwind's output), not in
  `style` blocks.
- The only acceptable inline block is a 1–3 line bootstrap that must run
  before paint (a theme-class toggle, an analytics snippet). Anything longer
  gets a file.
- Never wire behavior through `on*=` attributes — that's inline JS too.

## Text and content

- Short text goes in quotes at the end of the element's line: `span "Sign
  in"`. Multi-line text and text containing quotes use `|` lines.
- Text is HTML-escaped automatically — **write characters literally, never
  HTML entities**: `"Fenwick & Co."`, not `"Fenwick &amp; Co."` (the `&`
  would be double-escaped).
- Mixed inline content (text with elements mid-sentence) is written as
  sibling lines: text as `|` lines, elements as normal lines; an empty `|`
  preserves a meaningful space.
- Repeated **byte-identical** inline `<svg>` icons factor well into a def's
  body as raw `<` lines. Identical only — interpolation does not run inside
  raw lines.

## Readability

- 2-space indentation, always — `fhtml fmt` enforces it.
- Collapse single-child chains with `>`: `li > a(href=/docs) "Docs"`. Don't
  chain past one level of real content.
- Use `.` for a classless-tag div: `. flex gap-4`.
- Comment with `//` only where the *why* is non-obvious; `//!` when the
  comment should ship as an HTML comment.
- Past ~150 lines, split the file (see Project structure) before adding more.

## Reference files

- `references/language.md` — the complete syntax and template reference
  (canonical; same content the compiler's test suite pins).
- `references/examples.md` — annotated idiomatic examples: flat vs
  component-factored markup, and a multi-file partial layout.

## Quick checklist

Before calling any fhtml work done:

1. `fhtml fmt` run on every touched file?
2. `fhtml <file>` (or `fhtml build`) exits clean — with `--deny-warnings`?
3. Repeated markup factored into a `def`; single-use markup left plain?
4. Every string argument in every `+call` double-quoted (including URLs)?
5. No dead defs (each `def` called at least once)?
6. No inline `script`/`style` bodies beyond a tiny bootstrap; no `on*=`
   attributes?
7. Text written as literal characters, not HTML entities?
8. No class names built from expressions; conditionals interpolate whole
   class names?
9. Pages split into partials + `include` once they grow large?
10. Template tasks render from `--data` JSON, not hardcoded values?
