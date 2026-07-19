# Changelog

All notable changes to fhtml are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] — 2026-07-19

Conditional classes without a helper, and the batch build path as a
library call.

### Added

- **`compileFilesToDir` in `@fhtml/core/node`** — batch-compiles `.fhtml`
  entries into a directory of ES modules plus an `index.js` registry, with the
  correctness a dev loop needs baked in: no up-front wipe, temp-file +
  `rename()` for every output, index swapped last, manifest-tracked pruning
  that runs after the fresh index is live and only ever touches files the
  helper emitted, unchanged outputs skipped, and compile errors thrown before
  any write (`FhtmlError` gains an optional `file`). Emits a
  `"type": "module"` `package.json` into the output directory so the modules
  work inside CommonJS projects.

### Changed

- **Class-position falsiness (the clsx rule, SPEC §9.2)** — in class position
  (a bare interpolation token, or an interpolation inside a `class` attribute
  value), a result that is a boolean or falsy now emits no classes. With
  `&&`/`||` already yielding operand values, conditional classes read like
  JSX's `classnames()` with no helper: `{active && 'bg-indigo-600 text-white'}`
  adds the classes or nothing — previously a false guard emitted a literal
  `false` (or `0`) class. Class position only; stringification elsewhere is
  unchanged, and both backends agree byte-for-byte. Breaking in the narrow
  case of `{flag}` in class position, which used to emit `true`/`false`.

## [0.2.0] — 2026-07-19

Framework adapters for the JavaScript package. Both are subpaths of
`@fhtml/core` and neither imports its framework — the package stays
dependency-free.

### Added

- **`@fhtml/core/express`** — an Express view engine:
  `app.engine("fhtml", engine())`, then `res.render("page", locals)`. `init()`
  runs lazily on first render, Express's bookkeeping keys are filtered out of
  the template data, and with view caching on (production) each view's include
  closure is read from disk once.
- **`@fhtml/core/hono`** — a Hono renderer middleware:
  `c.render(name, data, ctx?)` over a bundled `{name: source}` file map, so it
  works on edge runtimes with no filesystem; Workers pass their native wasm
  import via the `wasm` option. The `.fhtml` extension may be omitted, like an
  `include` path.

### Changed

- `@fhtml/core` package metadata now links the repository and homepage.

## [0.1.0] — 2026-07-18

The first public release. The compiler, the template layer, components, the
tooling, and the JavaScript build are all in place; the core stays
zero-dependency.

### Added

- **Markup layer** — the whitespace language that compiles 1:1 to HTML.
  Indentation nests, bare tokens are classes copied byte-for-byte, attributes in
  parens, `.` for `div`, `#id`, quoted text and `|` text blocks, `>` inline
  children, `<` raw-HTML passthrough, `//` comments, `\` line continuation, and
  `doctype` (SPEC §1–§8, §11).
- **Template layer** — `{expr}`/`{!expr}` interpolation over a small closed
  expression language, `if`/`elif`/`else`, and `for`/`empty`, rendered against
  JSON `--data`/`--ctx` (SPEC §9–§10).
- **Composition** — `def` components with a `children` slot, `+name(args)`
  instantiation, and `include` for splicing files and sharing `def`s
  (SPEC §10.3–§10.5).
- **`--target=js` backend** — emits a self-contained
  `(data, ctx = {}) => string` ES module per file, byte-identical to the native
  renderer, with includes inlined so the module has no imports.
- **`fhtml fmt`** — canonical formatter (2-space indent, `.` for `div`, minimal
  quoting) that never changes compiled output.
- **`html2fhtml`** — the reverse converter (behind the `convert` feature), with
  `--check` round-trip verification and `--convert-svg`.
- **Class shorthand** (`#!shorthand`, SPEC §3.2) — an opt-in codebook that
  contracts common Tailwind utilities; `fmt --contract` / `--expand` move a file
  between forms with identical output.
- **`@fhtml/core`** — the compiler as WebAssembly (~261 KB) plus dependency-free
  ESM glue for Node, browsers, and edge runtimes; output is byte-identical to the
  CLI, and that parity is the release gate.
- **`vite-plugin-fhtml`** — imports `.fhtml` as a render function or `?html` as a
  static string, with compile errors in Vite's overlay and include-aware HMR.
- **Language server** — `analyze()`-backed diagnostics, formatting, document
  symbols, definitions, and completion, with a VS Code client.
- **Editor support** — a TextMate grammar and VS Code extension covering the full
  language, template layer included.
- **Tailwind v4** — verified `@source` scanning of `.fhtml` directly, arbitrary
  values and `data-[…]:` variants included.
- **Benchmark harness** (`bench/`) — token counts and round-trip fidelity over a
  48-component Tailwind corpus, plus an LLM generation-error benchmark.

### Benchmarks

- **−14%** tokens versus pretty HTML overall on the corpus (up to −20% on
  SVG-light markup), and **−9%** more with class shorthand.
- **48/48** components round-trip HTML → fhtml → identical DOM.
- **90.6%** of model-written fhtml completions compile, versus 44.3% for Pug,
  across four LLMs on the same corpus.

[0.3.0]: https://github.com/nft/fhtml/releases/tag/v0.3.0
[0.2.0]: https://github.com/nft/fhtml/releases/tag/v0.2.0
[0.1.0]: https://github.com/nft/fhtml/releases/tag/v0.1.0
