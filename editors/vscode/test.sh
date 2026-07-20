#!/bin/sh
# TextMate grammar tests: scope assertions + full-file snapshot.
# Plus the LSP-client smoke test (stubbed VS Code API — plain node, no setup).
#
# Setup (once): npm install  (vscode-tmgrammar-test is a devDependency)
# Pass --updateSnapshot to regenerate tests/snap/*.snap after grammar changes.
set -eu

cd "$(dirname "$0")"

node tests/client.test.cjs

# Prefer the lockfile-pinned devDependency (what CI installs); fall back to
# the shared bench/.tools install some local setups already have.
BIN=node_modules/.bin
[ -x "$BIN/vscode-tmgrammar-test" ] || BIN=../../bench/.tools/node_modules/.bin
[ -x "$BIN/vscode-tmgrammar-test" ] || {
  echo "vscode-tmgrammar-test missing — see setup line in this script"; exit 1;
}

# tests/html-stub.tmLanguage.json stands in for VS Code's built-in
# text.html.basic so the raw-passthrough include resolves outside an editor.
"$BIN/vscode-tmgrammar-test" \
  -g syntaxes/fhtml.tmLanguage.json -g tests/html-stub.tmLanguage.json \
  "tests/*.test.fhtml"
"$BIN/vscode-tmgrammar-snap" -s source.fhtml \
  -g syntaxes/fhtml.tmLanguage.json -g tests/html-stub.tmLanguage.json \
  "tests/snap/*.fhtml" "$@"
