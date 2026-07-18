# Changelog

All notable changes to fhtml are documented here. The format is based on
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.0]: https://github.com/nft/fhtml/releases/tag/v0.1.0
