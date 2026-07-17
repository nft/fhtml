# vite-plugin-fhtml

Import [fhtml](https://github.com/nft/fhtml) (Fluid HTML) files in
Vite. The plugin shells out to the `fhtml` CLI — the compiler stays the
single source of truth, and rendered output is byte-identical to
`fhtml --min` for the same file and data.

```js
// vite.config.js
import { defineConfig } from "vite";
import fhtml from "vite-plugin-fhtml";

export default defineConfig({
  plugins: [fhtml()],
});
```

## Import model

```js
// A render function — templated or static, always the same shape.
import render from "./card.fhtml";
document.body.innerHTML = render({ title: "Hello" });

// The compiled static HTML as a string (fhtml --static --min).
import hero from "./hero.fhtml?html";
```

The default import is the self-contained ES module emitted by
`fhtml --target=js`: `export default (data, ctx = {}) => string`, no runtime
dependency. A static file compiles to a constant function, so importers
never care which kind they got. `?html` runs the static path instead
(`fhtml --static`); a file that uses the template layer errors under
`?html` — rendering needs data.

## The fhtml binary

Resolution order: the `bin` plugin option → the `FHTML_BIN` environment
variable → `fhtml` on `$PATH`. Install it with `cargo install --path .` from
the fhtml repo.

```js
fhtml({ bin: "/path/to/fhtml" })
```

## Diagnostics and HMR

Compile errors surface as Rollup errors with a `loc`, so Vite's dev overlay
and build output point at the `.fhtml` line:column. Compiler warnings (for
example the Tailwind concat-class lint, SPEC §9.1) pass through as plugin
warnings.

Includes are watched transitively (via `fhtml deps`): editing a partial
invalidates every module that includes it.

## Tailwind

fhtml writes classes as bare tokens, so Tailwind v4 scans `.fhtml` sources
as-is — one `@source` line and no other configuration:

```css
@import "tailwindcss";
@source "./src/**/*.fhtml";
```

(Verified against the fhtml benchmark corpus: the CSS built from fhtml
sources covers every utility the equivalent HTML build finds, arbitrary
values and `data-[…]:` variants included — `bench/tailwind_scan.sh` in the
main repo.)

One rule keeps that true: **never build a class name from string parts**.
Tailwind's scanner is static — it can see `bg-blue-600` written out, but not
the result of `{"bg-" + color}`. The compiler enforces the rule for you:

- an interpolation *glued* to class text (`bg-{color}-100`) is a hard error;
- a class built with `+` concatenation compiles but **warns** (it's legal
  output, just invisible to Tailwind) — and the warning shows up right in
  Vite's console via this plugin.

Write whole class names and switch between them instead:

```
button {active ? "bg-blue-600 text-white" : "bg-gray-100 text-gray-900"}
```

`fhtml --deny-warnings` turns the warning into a failure for CI.

## Example

A complete hot-reloading Vite + Tailwind page (templated import, `?html`
import, an `include`d partial) lives in
[`example/`](example/) — `npm install && npm run dev`.

## Non-goals (v1)

No WASM build, no dev-server MPA mode (use `fhtml build` for static sites),
no framework wrappers, no data-loading conventions.
