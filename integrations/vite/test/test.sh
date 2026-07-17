#!/bin/sh
# Gate for vite-plugin-fhtml. Node-gated: exits 0
# with a skip message when node/npm are missing or too old, so node-free
# machines and CI lanes stay green. First run needs network for `npm install`
# (vite); the compiler itself is built from this repo and reached via
# FHTML_BIN, so $PATH needs nothing.
set -eu

cd "$(dirname "$0")"

if ! command -v node >/dev/null 2>&1 || ! command -v npm >/dev/null 2>&1; then
  echo "skip: node/npm not found — vite plugin test not run"
  exit 0
fi
major=$(node -p 'process.versions.node.split(".")[0]')
if [ "$major" -lt 20 ]; then
  echo "skip: vite 7 needs node >= 20 (have $(node --version))"
  exit 0
fi

repo=$(cd ../../.. && pwd)
(cd "$repo" && cargo build --quiet)
export FHTML_BIN="$repo/target/debug/fhtml"

[ -d node_modules ] || npm install --no-audit --no-fund
exec node run.mjs
