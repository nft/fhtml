#!/bin/sh
# Releases @fhtml/core. The artifact is
# built here, at release time — never on install: users get a prebuilt
# fhtml.wasm and no toolchain requirement. Steps: version parity across the
# core crate / wasm crate / package.json, the full test gate (raw ABI, glue
# API, corpus-wide byte-parity vs the native CLI), then a pack gate — the
# npm-pack tarball installed into a scratch project and cold-start smoked
# on both loaders — and only then `npm publish`.
#
#   ./release.sh --dry-run   everything except the publish
set -eu

cd "$(dirname "$0")"

dry=0
[ "${1:-}" = "--dry-run" ] && dry=1

# ---- version parity: package.json == core crate == wasm crate -------------

core=$(sed -n 's/^version = "\(.*\)"$/\1/p' ../../Cargo.toml | head -1)
wasm=$(sed -n 's/^version = "\(.*\)"$/\1/p' crate/Cargo.toml | head -1)
pkg=$(node -p "require('./package.json').version")
if [ "$core" != "$pkg" ] || [ "$core" != "$wasm" ]; then
  echo "version mismatch: core crate $core, wasm crate $wasm, package $pkg" >&2
  exit 1
fi
echo "release: @fhtml/core $pkg"

# ---- the full gate (builds the wasm and copies it to ./fhtml.wasm) --------

test/test.sh

# ---- pack gate: the tarball installs and cold-starts ----------------------

tarball=$(npm pack --silent)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"; rm -f "$tarball"' EXIT

# The README's own usage, verbatim: default loader on Node.
cat > "$tmp/smoke.mjs" << 'EOF'
import { init, render, version, FhtmlError } from "@fhtml/core";
import assert from "node:assert/strict";

await init(); // loads fhtml.wasm from next to the module; idempotent

const { html } = render('div grid\n  span rounded "hi"\n');
assert.equal(html, '<div class="grid"><span class="rounded">hi</span></div>');

const out = render(
  {
    "lib.fhtml": 'def badge(label)\n  span rounded "{label}"\n',
    "main.fhtml": 'include ./lib\n\ndiv grid\n  +badge(label={name})\n',
  },
  { entry: "main.fhtml", data: { name: "hi" }, mode: "pretty" },
);
assert.match(out.html, /rounded/);

try {
  render('span "unclosed\n');
  assert.fail("should have thrown");
} catch (e) {
  assert.ok(e instanceof FhtmlError);
  assert.equal(e.line, 1);
}
console.log(`smoke (default loader): ok, fhtml ${version()}`);
EOF

# Workers-style: raw bytes into init() — fresh process, no file: loading.
cat > "$tmp/smoke-bytes.mjs" << 'EOF'
import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";
import { init, render, version } from "@fhtml/core";

const bytes = await readFile(
  new URL("./node_modules/@fhtml/core/fhtml.wasm", import.meta.url),
);
await init(bytes);
const { html } = render('p "edge"\n');
assert.equal(html, "<p>edge</p>");
console.log(`smoke (init bytes): ok, fhtml ${version()}`);
EOF

here=$(pwd)
(
  cd "$tmp"
  npm install --no-audit --no-fund --silent "$here/$tarball"
  node smoke.mjs
  node smoke-bytes.mjs
)

# ---- publish --------------------------------------------------------------

if [ "$dry" = 1 ]; then
  echo "dry run: skipping npm publish"
  exit 0
fi
npm publish --access public
