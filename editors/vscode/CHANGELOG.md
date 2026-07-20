# Changelog

## 0.1.0 — 2026-07-20

Initial marketplace release.

- TextMate grammar covering the full language: element lines, `|` text blocks,
  comments, raw `<` HTML passthrough (embedded HTML), and the template layer.
- LSP client for the compiler's built-in `fhtml lsp`: diagnostics, formatting,
  outline, go-to-definition, completion. Requires the `fhtml` binary (0.2.0 or
  newer) on $PATH or via the `fhtml.path` setting. Only runs in trusted
  workspaces. A missing, outdated, or failing binary degrades quietly to
  highlighting-only with a single hint — never a restart loop or raw error.
