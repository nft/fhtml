# fhtml — reference for agents

fhtml is a whitespace-based markup language that compiles 1:1 to HTML,
designed for token-cheap generation: no closing tags, no angle brackets,
no `class="…"` wrappers. Bare tokens after a tag are the class list,
byte-for-byte — every Tailwind class works unquoted. Files end in
`.fhtml`.

Paste this file (or the sections you need) into a project's `CLAUDE.md`,
`AGENTS.md`, or `.cursorrules` to have an agent write correct fhtml.
The syntax and components sections are the exact prompts that were
benchmark-validated across multiple models (see `bench/RESULTS.md`);
`SPEC.md` is the normative definition.

## Syntax (complete)

Line shape: `tag(attrs) #id classes… "text"` — everything after the tag
is optional.

- **Indentation nests** (2 spaces per level, Python's rules). No closing tags.
- **Bare tokens are CSS classes**, copied verbatim: `p text-lg hover:bg-blue-500 w-1/2`
  → `<p class="text-lg hover:bg-blue-500 w-1/2">`. Any Tailwind class works unquoted,
  including `data-[state=open]:rotate-180` and `bg-[#0f172a]`.
- **Attributes go in parens butted against the tag** (no space): `a(href=/about target=_blank)`.
  Quote a value only if it contains spaces or parens: `img(alt="Team photo")`.
  Boolean attributes are bare: `input(type=checkbox checked)`.
- **`.` alone means `div`**: `. flex gap-4` → `<div class="flex gap-4">`.
- **`#id` as a bare token sets the id**: `nav #main-nav flex`.
- **Text is double-quoted at the end of the line**: `span "Sign in"`. It is
  HTML-escaped automatically — write characters literally, never HTML entities:
  `span "Fenwick & Co."`, not `span "Fenwick &amp; Co."` (the `&` would be
  escaped again, emitting `&amp;amp;`). Same in `|` text blocks.
- **`|` lines are text blocks** for multi-line text or text containing quotes:
  ```
  p text-sm
    | Multi-line text goes here,
    | one line per source line.
  ```
- **`>` chains a single child inline**: `li > a(href=/docs) "Docs"` →
  `<li><a href="/docs">Docs</a></li>`.
- **A line starting with `<` is raw HTML passthrough**, e.g. an inline `<svg>`:
  its whole indented subtree is emitted verbatim. Continuation lines of one raw
  element are indented 2 extra spaces.
- **`script`/`style` bodies are raw text**: `|` lines under them emit verbatim —
  no escaping, no interpolation.
- **Mixed inline content** (text with inline elements inside a sentence) is
  written as sibling lines: text as `|` lines, elements as normal lines. An
  empty `|` line preserves a meaningful space between a text line and the
  element that follows it.
- Void elements (`img`, `br`, `input`, `meta`, …) need no closing.
  `doctype` → `<!DOCTYPE html>`. `//` starts a comment (not emitted);
  `//!` is a comment that IS emitted as an HTML comment.

Example:

```
div flex items-center gap-4 rounded-xl bg-white p-6 shadow-md
  img(src=/img/ava.jpg alt="Erin's avatar") size-12 rounded-full
  .
    p text-lg font-semibold text-gray-900 "Erin Lindford"
    p text-gray-500 "Product Engineer"
  button ml-auto rounded-full px-4 py-1 text-sm hover:bg-purple-600 hover:text-white "Message"
```

## Templates

`{expr}` interpolation and statements render with JSON data (`--data`);
`ctx` is a second read-only root (`--ctx`). Missing names render as
null (empty text). Expressions are a small language: literals, `. []`
access, arithmetic, comparisons, `&& || !`, ternary — not JavaScript.

```
ul divide-y
  for item, i in items
    li py-2 {i % 2 == 0 ? 'bg-gray-50' : ''} "{i + 1}. {item.title}"
  empty
    li text-gray-400 "Nothing here yet."
if user.admin
  a(href=/admin) "Admin"
elif user.name
  span "{user.name}"
else
  a(href=/login) "Sign in"
```

- **Never build class names from expressions** — Tailwind's scanner is
  static. An interpolation glued to class text (`bg-{color}-100`) is a
  compile ERROR; string concatenation (`{"bg-" + color}`) compiles but
  warns. Interpolate whole class names instead:
  `button {active ? "bg-blue-600 text-white" : "bg-gray-100"}`.
- Inside `{…}`, string literals take single or double quotes:
  `{done ? 'Yes' : "No"}`.
- A literal `{` in text is escaped `\{`.

## Components

When the same markup shape repeats with only the text or attribute values
changing, factor it once with `def` and instantiate it with `+name(…)`:

```
def feature_card(title href badge_text=null)
  li rounded-xl bg-white p-6 shadow
    h3 text-lg font-semibold > a(href={href}) "{title}"
    if badge_text
      span rounded-full bg-indigo-50 px-2 text-xs "{badge_text}"
    p mt-2 text-sm text-gray-600
      children

ul grid grid-cols-3 gap-6
  +feature_card(title="Fast" href="/fast" badge_text="New")
    | Ships in milliseconds.
  +feature_card(title="Safe" href="/safe")
    | Every change is previewed.
```

- **Names use underscores, never hyphens.** Component and parameter names are
  expression identifiers (letters, digits, `_`); `-` is minus. `def
  blog-post(img-src)` is an ERROR — write `def blog_post(img_src)`.
- **`def name(param param=default)`** — top level only. The body sees ONLY its
  parameters (interpolate them: `{title}`); it cannot see other variables.
  Defaults follow call-argument quoting: `variant="emerald"`, not
  `variant=emerald` (that's a variable reference, which is null).
- **`+name(args)` instantiates.** Arguments are named-only. **Quoting differs
  from tag attributes — this is the one trap:** in a call, an unquoted value
  is an *expression*, not a string. `n=3` is the number 3, `wide=false` is a
  boolean, but `title=Fast` and `href=/fast` are ERRORS. **Every string
  argument must be double-quoted**, including URLs: `href="/fast"` (even
  though `a(href=/fast)` on a plain tag is fine).
  - A bare argument name means `true`: `+card(compact)`.
- **`children`** in the body marks where the caller's indented block goes.
  Put the longest varying content (a sentence, a paragraph) in the block
  instead of a parameter. A block is only allowed if the def uses `children`.
- A parameter without a default is required at every call.
- **Parameterize EVERY difference between the repeats** — ids, `aria-label`s,
  a `checked` flag, the selected item's extra classes. Compare the instances
  token by token; a difference you flatten away corrupts the output. If the
  blocks differ in structure, leave them plain.
- Byte-identical repeated `<svg>` icons factor well: put the raw `<svg …>`
  lines inside a def's body. Identical ONLY — interpolation does not run
  inside raw `<` lines, so a `{path_d}` there stays literal text.
- A text-only child line starts with `|`. A bare quoted line is an ERROR —
  quotes only attach text to an element's own line.
- Markup that does **not** repeat stays plain — never wrap single-use markup
  in a def; that costs tokens instead of saving them.
- `include ./partials/head` splices another file: its `def`s join the
  namespace, its markup emits at the include site. Paths are relative to
  the including file; cycles and `def` collisions are errors.

## Toolchain

The intended loop is **write → `fhtml fmt` → build**. Compile errors
carry `line:col`; fix and re-run.

```sh
fhtml page.fhtml                     # compile to stdout (minified)
fhtml page.fhtml --data data.json    # render the template layer
fhtml build src/ -o dist/            # compile a directory tree
fhtml fmt src/                       # canonical formatting, in place
fhtml page.fhtml --deny-warnings     # CI: any warning fails the build
fhtml build src/ -o dist --target=js # ES modules: (data, ctx={}) => string
```

- Always generate plain Tailwind classes. Never emit `#!shorthand`
  codes — the shorthand codebook is a write-time storage compression
  (`fhtml fmt --contract`), not an output format.
- From JavaScript: `npm install @fhtml/core` (WebAssembly, runs on
  Node/Bun/Deno/Workers/browsers) — `render`, `compileToJs`, `format`,
  `analyze`; on Node, `@fhtml/core/node` exports `renderFile` and
  friends; `@fhtml/core/express` is an Express view engine,
  `@fhtml/core/hono` a Hono renderer middleware. Vite:
  `vite-plugin-fhtml` imports `.fhtml` files as render functions.
- Editor support: `fhtml lsp` (diagnostics, formatting, go-to-def,
  completion) with a VS Code extension under `editors/vscode/`.
