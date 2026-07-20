# @fhtml/core

[Website](https://nft.github.io/fhtml/) · [Docs](https://nft.github.io/fhtml/docs.html) · [Repository](https://github.com/nft/fhtml)

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

### Compile a views directory

The build-time path this README recommends, as one call —
`compileFilesToDir` compiles a set of views into a directory of ES
modules plus an `index.js` registry, safely for a live dev loop:

```js
import { init } from "@fhtml/core";
import { compileFilesToDir } from "@fhtml/core/node";

await init();
compileFilesToDir({
  entries: ["views/card.fhtml", "views/page.fhtml"], // or {name: path}
  outDir: "src/generated",
});
```

```js
import views from "./generated/index.js";
res.end(views.card({ name: "hi" })); // zero wasm at request time
```

Its guarantees are the reason it exists in the library instead of in
every project's build script: the output directory is **never wiped**;
every file lands via temp-file + `rename()`, so a watcher (`bun --hot`,
Vite, tsx) can never observe a missing or half-written module;
`index.js` — the completion signal — is swapped **last**, after every
module it imports exists; pruning of removed views is tracked in a
`.fhtml-manifest.json` and runs only after the fresh index is live, and
only ever deletes files the helper itself emitted; unchanged outputs
are skipped, so watchers of `outDir` don't rebuild in a loop; a compile
error throws (a `FhtmlError` with `.file` set) **before any write**. A
`package.json` with `"type": "module"` is emitted into `outDir`, so the
generated modules work inside CommonJS projects too. Options:
`mode`, `emitIndex`, `emitDts`, `prune` (all defaulting on); returns
`{written, unchanged, pruned, warnings}`.

Scope, stated plainly: one writer per `outDir` at a time; atomicity is
per file, on ordinary local filesystems (no fsync — not a durability
guarantee); a reader still holding an older index is not protected from
pruning; files at generated output paths get replaced (untracked files
are never *pruned*, though). No watch mode by design — atomic writes
are what make *your* watcher safe.

### Express

A view engine, one registration away (`express` itself is never
imported — the engine is just a callback):

```js
import express from "express";
import { engine } from "@fhtml/core/express";

const app = express();
app.engine("fhtml", engine());
app.set("view engine", "fhtml");

app.get("/", (req, res) => res.render("page", { name: "hi" }));
```

Render locals become the template `data`; `engine({ctx, mode,
onWarnings})` sets the per-app knobs. `init()` runs lazily on the first
render. When Express enables view caching (`NODE_ENV=production`), each
view's include closure is read from disk once and reused.

### Hono

A renderer middleware — templates come in as the usual file map, so it
works on edge runtimes with no filesystem (`hono` itself is never
imported):

```js
import { Hono } from "hono";
import { fhtmlRenderer } from "@fhtml/core/hono";
import wasm from "@fhtml/core/fhtml.wasm"; // Workers; omit where the default loader works

const app = new Hono();
app.use(fhtmlRenderer(templates, { wasm }));

app.get("/", (c) => c.render("page", { name: "hi" }));
```

`c.render(name, data, ctx?)` renders `templates[name]` (the `.fhtml`
extension may be omitted, like an `include` path) and responds via
`c.html`. Compile errors throw `FhtmlError` into Hono's `onError`.

For both adapters the render happens in wasm per request — convenient,
and plenty fast. The zero-wasm-at-request-time path is still build-time
`compileToJs`.

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
