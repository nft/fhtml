# Changelog

## 0.1.0 — 2026-07-20

Initial marketplace release.

- TextMate grammar covering the full language: element lines, `|` text blocks,
  comments, raw `<` HTML passthrough (embedded HTML), and the template layer.
- LSP client for the compiler's built-in `fhtml lsp`: diagnostics, formatting,
  outline, go-to-definition, completion. Requires the `fhtml` binary on $PATH
  or via the `fhtml.path` setting. Only runs in trusted workspaces.
