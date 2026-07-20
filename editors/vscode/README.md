# fhtml for VS Code

[Website](https://nft.github.io/fhtml/) · [Docs](https://nft.github.io/fhtml/docs.html) · [Repository](https://github.com/nft/fhtml)

Language support for [fhtml (Fluid HTML)](https://github.com/nft/fhtml) — a
whitespace-based markup language that compiles 1:1 to HTML.

- **Syntax highlighting** — the whole language, including embedded HTML and the
  template layer. Works out of the box, no compiler needed.
- **Language server** — diagnostics, formatting, outline, go-to-definition, and
  completion, once the `fhtml` compiler is installed.

## Install

Install **fhtml** from the [VS Code Marketplace](https://marketplace.visualstudio.com/items?itemName=fhtml.fhtml)
(`ext install fhtml.fhtml`) or from [Open VSX](https://open-vsx.org/extension/fhtml/fhtml)
for VSCodium and Cursor.

Highlighting works immediately. For the language server, install the compiler
(0.2.0 or newer):

```sh
cargo install --git https://github.com/nft/fhtml
```

The extension runs `fhtml` from your `$PATH`; set `fhtml.path` to point at a
specific binary. Without the compiler you still get highlighting, and the
server stays off in Restricted Mode (untrusted) workspaces.

## Language server

Provided by the compiler's built-in `fhtml lsp`:

- **Diagnostics** — errors and warnings, live on every keystroke.
- **Formatting** — matches `fhtml fmt`, so format-on-save works.
- **Outline** — components and includes.
- **Go-to-definition** — jump from a `+call` to its `def`, across files.
- **Completion** — components, parameters, keywords, and HTML tags.

## Tailwind CSS

Class completions come from [Tailwind CSS IntelliSense](https://marketplace.visualstudio.com/items?itemName=bradlc.vscode-tailwindcss).
fhtml writes classes as bare tokens on the element line, so tell it where they
live:

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

## Development

```sh
npm install        # once — installs vscode-tmgrammar-test
./test.sh          # client smoke test + grammar scope assertions + snapshot
```

The grammar (`syntaxes/fhtml.tmLanguage.json`, scope `source.fhtml`) is a
standard TextMate grammar, reusable anywhere TextMate grammars are. To hack on
the extension from source, symlink `editors/vscode/` into `~/.vscode/extensions/`.
