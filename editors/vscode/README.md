# fhtml for VS Code

Language support for [fhtml (Fluid HTML)](https://github.com/nft/fhtml) `.fhtml` files:
syntax highlighting always, and — when the fhtml compiler is installed —
live diagnostics, formatting, outline, go-to-definition and completion via
the built-in language server (`fhtml lsp`).

The highlighting covers the whole language: element lines (tag, `(attrs)`,
`#id`, verbatim Tailwind classes, `"text"`, `>` chains, `\` continuation),
`|` text blocks, `//` / `//!` comments with their indented continuation
lines, raw `<` HTML passthrough (highlighted as real HTML), `doctype`, and
the template layer — `{expr}` interpolation with the SPEC §9.3 expression
grammar, `if`/`elif`/`else`, `for`/`empty`, `def`/`children`,
`+component(calls)`, `include`.

## Install

Install **fhtml** from the
[VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=fhtml.fhtml)
(`ext install fhtml.fhtml`) or, for VSCodium/Cursor-family editors, from
[Open VSX](https://open-vsx.org/extension/fhtml/fhtml).

Indentation-based folding and `//` line comments are configured in
`language-configuration.json`.

From source (contributors): in `editors/vscode/` run
`npm install` (fetches `vscode-languageclient`, the only runtime
dependency), then `ln -s "$(pwd)" ~/.vscode/extensions/fhtml-0.1.0` and
reload VS Code.

## Language server

The extension spawns `fhtml lsp` (the compiler's built-in, zero-dependency
language server) for every workspace with `.fhtml` files open. It provides:

- **diagnostics** — the parse error and all warnings (including the
  dynamic-class-fragment lint), live on every keystroke;
- **formatting** — identical output to `fhtml fmt`, so format-on-save works;
- **outline** (`documentSymbol`) — `def`s with their parameters, `include`s;
- **go-to-definition** — F12 on a `+call` jumps to its `def`, including
  across `include`d files; F12 on an `include` path opens the file;
- **completion** — component names after `+`, a call's remaining parameters
  inside `(…)`, statement keywords and HTML tags at line start.

The binary is looked up as `fhtml` on `$PATH`; the `fhtml.path` setting
overrides that with an explicit path. If it isn't found, the extension says
so once and stays in highlighting-only mode — install the compiler with
`cargo install --git https://github.com/nft/fhtml` (or
`cargo install --path .` from a clone; add `--features convert` if you also
want `html2fhtml`). Note the compiler is not the `fhtml` crate on crates.io
(an unrelated project). The server only runs in trusted workspaces: in
Restricted Mode you get highlighting only, and a workspace-provided
`fhtml.path` is ignored until you trust the workspace.

## Tailwind CSS completions

Class completions are deliberately **not** reimplemented here — pair with
[Tailwind CSS IntelliSense](https://marketplace.visualstudio.com/items?itemName=bradlc.vscode-tailwindcss),
which owns the class vocabulary. fhtml writes classes as bare tokens on the
element line (`div rounded-xl p-6`), so Tailwind IntelliSense needs to be
told where classes live. In your settings:

```json
{
  "tailwindcss.includeLanguages": { "fhtml": "html" },
  "tailwindcss.experimental.classRegex": [
    [
      "(?:^|\\n)[ \\t]*(?:[A-Za-z][\\w-]*|\\.)(?:\\([^)\\n]*\\))?(?:#[^\\s\"{}]+)?((?:[ \\t]+[^\\s\"{}]+)+)",
      "([^\\s\"{}]+)"
    ]
  ]
}
```

The first regex captures the bare-token run after the tag (or the `.` div
shorthand), skipping a `(attrs)` group and `#id`; the second extracts each
candidate token — quoted text and `{expr}` interpolations never match. It
is best-effort by design: after a `>` chain or a `\` continuation the
capture also covers the continuation tokens, which is harmless — Tailwind
only completes tokens that look like its own classes.

## Development

```sh
node tests/client.test.cjs    # LSP-client smoke test (stubbed VS Code API)

npm install                   # once — vscode-tmgrammar-test is a devDependency
./test.sh                     # client test + scope assertions + snapshot
./test.sh --updateSnapshot    # after intentional grammar changes
```

The grammar itself (`syntaxes/fhtml.tmLanguage.json`, scope `source.fhtml`)
is a standard TextMate grammar, reusable by anything TextMate-compatible
(Sublime Text via conversion, GitHub Linguist once the language qualifies,
`shiki`/`starry-night` for static site highlighting).

`tests/basic.test.fhtml` pins the load-bearing claims (Tailwind arbitrary
values stay one class token, quoted attr values may contain parens, `\`
continuation keeps class scope, comments swallow their indented children);
`tests/snap/kitchen-sink.fhtml.snap` is a reviewed full-file snapshot;
`tests/client.test.cjs` pins the client's spawn/missing-binary behavior.
