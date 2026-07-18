# @fhtml/core

The [fhtml](../..) compiler compiled to WebAssembly: render, compile to
plain JS modules, format, and analyze fhtml anywhere JavaScript runs —
Node, browsers, Cloudflare Workers, Deno. No native binary, no
postinstall step, no dependencies; the package is ~100 lines of ESM glue
plus a **261 KB** `fhtml.wasm` (the zero-dependency Rust core; soft
budget ≤ 500 KB).

## Install

```sh
npm install @fhtml/core
```

Runs on Node ≥ 18, Bun, Deno, Cloudflare Workers, and browsers — the
glue uses only standard APIs (`WebAssembly`, `TextEncoder`,
`import.meta.url`, `fetch` / `node:fs`). The test suite runs on Node
and, when installed, Bun; on Bun the byte-parity sweep against the
native CLI holds identically.

## Use

```js
import { init, render, compileToJs, format, analyze } from "@fhtml/core";

await init(); // loads fhtml.wasm from next to the module; idempotent

// A single source string…
const { html } = render('div grid\n  span rounded "hi"\n');

// …or a file map with includes, data, and ctx:
const out = render(
  {
    "lib.fhtml": 'def badge(label)\n  span rounded "{label}"\n',
    "main.fhtml": 'include ./lib\n\ndiv grid\n  +badge(label={name})\n',
  },
  { entry: "main.fhtml", data: { name: "hi" }, mode: "pretty" },
);
```

- `render(files, {entry, data, ctx, mode})` → `{html, warnings}`
- `compileToJs(files, {entry, mode})` → `{js, warnings}` — a
  self-contained ES module exporting `(data, ctx = {}) => string`, so
  the request-time render path carries zero wasm
- `format(src, {shorthand})` → formatted source
- `analyze(files, {entry})` → defs / calls / includes / diagnostics —
  the same data the native LSP serves
- `version()` → the compiler version

Compile and render errors throw `FhtmlError` with 1-based `line`/`col`
(null for non-source errors). `analyze` never throws on broken source —
the error comes back in `analysis.error`.

The wasm is a *build-time* compiler. For serving, prefer
`compileToJs` at build time and run the emitted module — it is plain
ESM with no runtime dependency on this package.

### On Node: skip the file map

The `@fhtml/core/node` subpath does the map-building for you — it reads
a file and its transitive `include`s from disk (include discovery runs
through the compiler's own `analyze`, so path semantics can't drift):

```js
import { init } from "@fhtml/core";
import { renderFile, compileFileToJs, analyzeFile, loadFiles } from "@fhtml/core/node";

await init();
const { html } = renderFile("views/page.fhtml", { data: { user: "Erin" } });
```

`loadFiles(path)` returns the raw `{files, entry}` pair for custom
flows. The subpath is Node-only by design — the root export never
imports `node:*`, so browser and edge bundles stay clean.

### Edge runtimes

Where the default loader can't reach the file (no `file:` URL, no
`fetch` for assets), pass the bytes or a compiled module yourself:

```js
import wasm from "@fhtml/core/fhtml.wasm"; // Workers-style native wasm import
await init(wasm);                          // also accepts raw bytes
```

## Build from source

```sh
cd crate && cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/fhtml_wasm.wasm ../fhtml.wasm
```

Requires the `wasm32-unknown-unknown` target
(`rustup target add wasm32-unknown-unknown`).

## Tests

`test/test.sh` — node-gated. Runs the raw ABI contract test (bare
`WebAssembly` API, memory-hygiene loop), the glue API tests, and the
parity gate: over every corpus file, wasm output must be byte-identical
to the native CLI for `render` (min and pretty), `compileToJs`, and
`format`.

## Release

`./release.sh` — checks version parity (package == core crate == wasm
crate), runs the full test gate, `npm pack`s and cold-start-smokes the
tarball in a scratch project (both loaders), then publishes. The
artifact is built here, at release time — never on install.
`--dry-run` stops before the publish; `--publish-only` skips straight
to the publish of an already-gated artifact (run it from a real
terminal so npm's 2FA can open the browser instead of demanding
`--otp`).
