<p align="center">
  <img src="fhtml.png" alt="fhtml — Fluid Hypertext Markup Language" width="480">
</p>

<p align="center">
  <a href="https://nft.github.io/fhtml/"><b>Website</b></a> ·
  <a href="SPEC.md">Spec</a> ·
  <a href="bench/RESULTS.md">Benchmark</a>
</p>

# fhtml — Fluid HTML

A whitespace-based markup language (`.fhtml`) that compiles 1:1 to HTML. Like Pug, but built
for two things Pug wasn't:

- **Token-cheap LLM/agent output** — no closing tags, no angle brackets, no `class="…"`
  wrappers. Measured on a 48-component Tailwind corpus: **14% fewer tokens than pretty
  HTML** overall, 20–25% on markup that isn't dominated by inline SVG payload
  ([bench/RESULTS.md](bench/RESULTS.md)).
- **Tailwind-native** — bare tokens after the tag *are* the class list, copied to output
  byte-for-byte. `hover:bg-blue-500`, `w-1/2`, `data-[state=open]:bg-red-500` — no escaping,
  ever.

```fhtml
div flex items-center gap-4 rounded-xl bg-white p-6 shadow-md
  img(src=/img/ava.jpg alt="Erin's avatar") size-12 rounded-full
  .
    p text-lg font-semibold text-gray-900 "Erin Lindford"
    p text-gray-500 "Product Engineer"
  button ml-auto rounded-full px-4 py-1 text-sm hover:bg-purple-600 hover:text-white "Message"
```

compiles to

```html
<div class="flex items-center gap-4 rounded-xl bg-white p-6 shadow-md">
  <img src="/img/ava.jpg" alt="Erin's avatar" class="size-12 rounded-full">
  <div>
    <p class="text-lg font-semibold text-gray-900">Erin Lindford</p>
    <p class="text-gray-500">Product Engineer</p>
  </div>
  <button class="ml-auto rounded-full px-4 py-1 text-sm hover:bg-purple-600 hover:text-white">Message</button>
</div>
```

## The rules in 30 seconds

Line shape: `tag(attrs) #id classes… "text"` — everything after the tag is optional.

- **Indentation nests** (Python's rules exactly); no closing tags.
- **Bare tokens are classes**, verbatim. The compiler never parses inside a class token.
- **Attributes live in parens**, butted against the tag: `a(href=/about target=_blank)`.
- **`.` alone means `div`**; `#id` as a token sets the id.
- **Text is quoted** (`span "Sign in"`), HTML-escaped; `|` lines for text blocks.
- **`li > a(href=/docs) "Docs"`** chains a single inline child.
- **A line starting with `<`** is raw HTML passthrough — the escape hatch.
- `\` at end of line continues it; `//` comments; `doctype` → `<!DOCTYPE html>`.

That's the whole markup layer. [SPEC.md](SPEC.md) is the normative definition.

## Install

Rust toolchain required (no other dependencies):

```sh
cargo install --path .                     # the fhtml compiler (zero-dep)
cargo install --path . --features convert  # + the html2fhtml converter
```

## Usage

```sh
fhtml page.fhtml                 # compile to stdout (minified)
echo 'p "hi"' | fhtml            # stdin → stdout, pipeline-friendly
fhtml page.fhtml -o page.html    # compile to a file (pretty)
fhtml build src/ -o dist/        # compile a directory tree of .fhtml files
fhtml fmt src/                   # reformat to canonical style, in place
```

`--pretty` / `--min` override the defaults (pretty when writing files, minified on stdout).
Errors carry line and column; non-fatal hazards (e.g. uneven indent steps) are warnings on
stderr.

### Templates

`{expr}` interpolation and `if`/`elif`/`else`, `for`/`empty` statements render with JSON
data (SPEC §9–§10):

```fhtml
ul divide-y
  for item, i in items
    li py-2 {i % 2 == 0 ? 'bg-gray-50' : ''} "{i + 1}. {item.title}"
  empty
    li text-gray-400 "Nothing here yet."
```

```sh
fhtml page.fhtml --data data.json            # render with data
fhtml page.fhtml --data d.json --ctx c.json  # + the read-only `ctx` root
fhtml build src/ -o dist/ --target=js        # emit ES modules instead of HTML
fhtml page.fhtml --no-templates              # enforce pure static markup
```

Without `--data`, template files render with every name `null`. `--target=js` emits a
self-contained ES module per file exporting `(data, ctx = {}) => string` — no imports, no
runtime dependency, byte-identical output to the native renderer:

```js
import render from "./dist/page.js";
document.body.innerHTML = render({ items: [{ title: "Ship it" }] });
```

`fhtml fmt` normalizes to 2-space indentation, `.` for `div`, and minimal quoting.
Formatting never changes the compiled output. The intended agent workflow is
*write → fmt → build*.

### html2fhtml

The reverse direction, for migrating existing markup (requires the `convert` feature):

```sh
html2fhtml page.html                # HTML → fhtml on stdout
html2fhtml src/ -o out/             # convert a directory tree (.html/.htm → .fhtml)
html2fhtml --check page.html        # verify the round-trip: HTML → fhtml → same DOM
html2fhtml --fragment=table row.html  # parse as a fragment (e.g. bare <tr>)
```

Output is always canonical (`fhtml fmt` on it is a no-op). Anything fhtml can't express
natively (exotic attribute names, `<svg>` by default) falls back to raw HTML lines, with a
warning on stderr; `--convert-svg` converts SVG subtrees instead.

### As a library

```rust
use fhtml::{compile, render, json, Mode};

let html = compile("p text-lg \"Hello\"", Mode::Pretty)?;

let data = json::parse(r#"{"name": "Erin"}"#)?;
let html = render("p \"Hi, {name}\"", &data, Mode::Min)?;
```

`compile` is the static path (template constructs are an error there); `render`/`render_full`
evaluate the template layer; `compile_to_js` emits the ES-module target; `format` reformats
source to canonical form; the `_full` variants also return warnings.

## Tailwind integration

Tailwind v4's scanner picks up fhtml classes as-is — they're plain space-separated tokens:

```css
@source "./src/**/*.fhtml";
```

Verified against tailwindcss v4.3.2 (`bench/tailwind_scan.sh`): CSS built from the benchmark
corpus as fhtml covers every utility the HTML build finds, arbitrary values and `data-[…]:`
variants included.

One rule: never build class names from expressions (`bg-{color}-100` is invisible to
Tailwind's static scanner). Interpolate whole class names instead.

## Editor support

A TextMate grammar and VS Code extension live in [editors/vscode/](editors/vscode/)
(no marketplace listing yet — symlink into `~/.vscode/extensions`, see its README).
The grammar covers the full language including the template layer, and raw `<` lines
are highlighted as embedded HTML.

## License

MIT
