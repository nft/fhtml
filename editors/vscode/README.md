# fhtml for VS Code

Syntax highlighting for [fhtml (Fluid HTML)](../../README.md) `.fhtml` files.

Covers the whole language: element lines (tag, `(attrs)`, `#id`, verbatim
Tailwind classes, `"text"`, `>` chains, `\` continuation), `|` text blocks,
`//` / `//!` comments with their indented continuation lines, raw `<` HTML
passthrough (highlighted as real HTML), `doctype`, and the template layer —
`{expr}` interpolation with the SPEC §9.3 expression grammar, `if`/`elif`/
`else`, `for`/`empty`, `def`/`children`, `+component(calls)`, `include`.

## Install (local — not on the marketplace yet)

From this directory:

```sh
ln -s "$(pwd)" ~/.vscode/extensions/fhtml-0.1.0
```

then reload VS Code. Indentation-based folding and `//` line comments are
configured in `language-configuration.json`.

The grammar itself (`syntaxes/fhtml.tmLanguage.json`, scope `source.fhtml`)
is a standard TextMate grammar, reusable by anything TextMate-compatible
(Sublime Text via conversion, GitHub Linguist once the language qualifies,
`shiki`/`starry-night` for static site highlighting).

## Tests

```sh
npm install --prefix ../../bench/.tools vscode-tmgrammar-test   # once
./test.sh                     # scope assertions + snapshot comparison
./test.sh --updateSnapshot    # after intentional grammar changes
```

`tests/basic.test.fhtml` pins the load-bearing claims (Tailwind arbitrary
values stay one class token, quoted attr values may contain parens, `\`
continuation keeps class scope, comments swallow their indented children);
`tests/snap/kitchen-sink.fhtml.snap` is a reviewed full-file snapshot.
