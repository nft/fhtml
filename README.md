# fhtml — Fluid HTML

A whitespace-based markup language (`.fhtml`) that compiles 1:1 to HTML. Like Pug, but built
for two things Pug wasn't:

- **Token-cheap LLM/agent output** — no closing tags, no angle brackets, no `class="…"`
  wrappers. Roughly 25–40% fewer output tokens on class-heavy markup.
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
cargo install --path .
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

`fhtml fmt` normalizes to 2-space indentation, `.` for `div`, and minimal quoting.
Formatting never changes the compiled output. The intended agent workflow is
*write → fmt → build*.

### As a library

```rust
use fhtml::{compile, Mode};

let html = compile("p text-lg \"Hello\"", Mode::Pretty)?;
```

`compile_full` additionally returns warnings; `format` reformats source to canonical form.

## Tailwind integration

Tailwind v4's scanner picks up fhtml classes as-is — they're plain space-separated tokens:

```css
@source "./src/**/*.fhtml";
```

One rule: never build class names from expressions (`bg-{color}-100` is invisible to
Tailwind's static scanner). Interpolate whole class names instead.

## License

MIT
