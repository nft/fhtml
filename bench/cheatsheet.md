# fhtml syntax (complete reference)

fhtml compiles 1:1 to HTML. Line shape: `tag(attrs) #id classes… "text"` —
everything after the tag is optional.

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
  HTML-escaped automatically.
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
