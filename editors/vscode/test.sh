#!/bin/sh
# TextMate grammar tests: scope assertions + full-file snapshot.
#
# Setup (once): npm install --prefix ../../bench/.tools vscode-tmgrammar-test
# Pass --updateSnapshot to regenerate tests/snap/*.snap after grammar changes.
set -eu

cd "$(dirname "$0")"
BIN=../../bench/.tools/node_modules/.bin
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
