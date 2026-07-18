#!/bin/sh
# Gate for the npm wasm package.
# Node-gated like integrations/vite/test/test.sh: exits 0 with a skip
# message when node or the wasm32 target is missing, so node-free machines
# and CI lanes stay green. Builds both sides itself: the wasm artifact
# (copied to ../fhtml.wasm, where index.js loads it from) and the native
# CLI the parity sweep compares against.
set -eu

cd "$(dirname "$0")"

if ! command -v node >/dev/null 2>&1; then
  echo "skip: node not found — npm package test not run"
  exit 0
fi
if ! rustup target list --installed 2>/dev/null | grep -q '^wasm32-unknown-unknown$'; then
  echo "skip: wasm32-unknown-unknown target not installed (rustup target add wasm32-unknown-unknown)"
  exit 0
fi

repo=$(cd ../../.. && pwd)

(cd ../crate && cargo build --quiet --release --target wasm32-unknown-unknown)
cp ../crate/target/wasm32-unknown-unknown/release/fhtml_wasm.wasm ../fhtml.wasm

(cd "$repo" && cargo build --quiet)
export FHTML_BIN="$repo/target/debug/fhtml"

node raw-abi.mjs
node api.mjs
node node-api.mjs
node parity.mjs

# The same suite under Bun when it's on the PATH — the package claims
# every standard-API runtime, so hold it to the claim where we can.
if command -v bun >/dev/null 2>&1; then
  echo "--- bun $(bun --version)"
  bun raw-abi.mjs
  bun api.mjs
  bun node-api.mjs
  bun parity.mjs
else
  echo "note: bun not found — bun lane skipped"
fi
