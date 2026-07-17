# @fhtml/core

The [fhtml](../..) compiler compiled to WebAssembly: render, compile to
plain JS modules, format, and analyze fhtml anywhere JavaScript runs —
Node, browsers, Cloudflare Workers, Deno. No native binary, no
postinstall step, no dependencies; the package is ~100 lines of ESM glue
plus a **261 KB** `fhtml.wasm` (the zero-dependency Rust core; soft
budget ≤ 500 KB).

> Package name and publishing are pending; `private` is
> set until then. Build the artifact locally with the command below.

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

### Edge runtimes

Where the default loader can't reach the file (no `file:` URL, no
`fetch` for assets), pass the bytes or a compiled module yourself:

```js
import wasm from "./fhtml.wasm"; // Workers-style native wasm import
await init(wasm);                // also accepts raw bytes
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
