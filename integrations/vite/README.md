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
as-is:

```css
@import "tailwindcss";
@source "./src/**/*.fhtml";
```

## Non-goals (v1)

No WASM build, no dev-server MPA mode (use `fhtml build` for static sites),
no framework wrappers, no data-loading conventions.
