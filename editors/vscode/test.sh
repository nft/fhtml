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

# tests/*-stub.tmLanguage.json stand in for VS Code's built-in grammars
# (text.html.basic, source.js/css/json) so the raw-passthrough and raw-text
# includes resolve outside an editor. Stubs are not cosmetic: an include of
# an unregistered scope silently disables its whole rule in vscode-textmate,
# so without them the raw-text rules never fire in tests.
STUBS="-g tests/html-stub.tmLanguage.json -g tests/js-stub.tmLanguage.json \
  -g tests/css-stub.tmLanguage.json -g tests/json-stub.tmLanguage.json"
"$BIN/vscode-tmgrammar-test" \
  -g syntaxes/fhtml.tmLanguage.json $STUBS \
  "tests/*.test.fhtml"
"$BIN/vscode-tmgrammar-snap" -s source.fhtml \
  -g syntaxes/fhtml.tmLanguage.json $STUBS \
  "tests/snap/*.fhtml" "$@"
