#!/bin/sh
# Releases @fhtml/core. The artifact is
# built here, at release time — never on install: users get a prebuilt
# fhtml.wasm and no toolchain requirement. Steps: version parity across the
# core crate / wasm crate / package.json, the full test gate (raw ABI, glue
# API, corpus-wide byte-parity vs the native CLI), then a pack gate — the
# npm-pack tarball installed into a scratch project and cold-start smoked
# on both loaders — and only then `npm publish`.
#
#   ./release.sh --dry-run        everything except the publish
#   ./release.sh --publish-only   skip the gates, publish the built artifact —
#                                 for finishing a release whose gates just
#                                 passed. Run it from a real terminal: with a
#                                 TTY, npm's 2FA opens the browser (passkey)
#                                 instead of demanding a typed --otp.
set -eu

cd "$(dirname "$0")"

dry=0
publish_only=0
case "${1:-}" in
  --dry-run) dry=1 ;;
  --publish-only) publish_only=1 ;;
esac

# ---- version parity: package.json == core crate == wasm crate -------------

core=$(sed -n 's/^version = "\(.*\)"$/\1/p' ../../Cargo.toml | head -1)
wasm=$(sed -n 's/^version = "\(.*\)"$/\1/p' crate/Cargo.toml | head -1)
pkg=$(node -p "require('./package.json').version")
if [ "$core" != "$pkg" ] || [ "$core" != "$wasm" ]; then
  echo "version mismatch: core crate $core, wasm crate $wasm, package $pkg" >&2
  exit 1
fi
echo "release: @fhtml/core $pkg"

if [ "$publish_only" = 1 ]; then
  if [ ! -f fhtml.wasm ]; then
    echo "no fhtml.wasm — run ./release.sh (the full gate) first" >&2
    exit 1
  fi
  npm publish --access public
  exit 0
fi

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

# The node subpath: renderFile builds the include closure from disk.
# Then the framework adapters, driven bare — proves all four subpaths
# resolve from the packed tarball.
cat > "$tmp/smoke-node.mjs" << 'EOF'
import { mkdirSync, writeFileSync } from "node:fs";
import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";
import { init } from "@fhtml/core";
import { compileFilesToDir, renderFile } from "@fhtml/core/node";
import { engine } from "@fhtml/core/express";
import { fhtmlRenderer } from "@fhtml/core/hono";

await init();
mkdirSync("views", { recursive: true });
writeFileSync("views/lib.fhtml", 'def badge(label)\n  span rounded "{label}"\n');
writeFileSync("views/page.fhtml", "include ./lib\n\n+badge(label={name})\n");
const { html } = renderFile("views/page.fhtml", { data: { name: "hi" } });
assert.equal(html, '<span class="rounded">hi</span>');

compileFilesToDir({ entries: ["views/page.fhtml"], outDir: "generated" });
const { views } = await import(pathToFileURL("generated/index.js"));
assert.equal(views.page({ name: "hi" }), html);

const viaEngine = await new Promise((res, rej) =>
  engine()("views/page.fhtml", { name: "hi" }, (e, h) => (e ? rej(e) : res(h))));
assert.equal(viaEngine, html);

const c = { setRenderer(fn) { this.render = fn; }, html: (s) => s };
await fhtmlRenderer({ "p.fhtml": 'p "edge"\n' })(c, async () => {});
assert.equal(c.render("p"), "<p>edge</p>");
console.log("smoke (node subpath + adapters): ok");
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
  node smoke-node.mjs
  node smoke-bytes.mjs
)

# ---- publish --------------------------------------------------------------

if [ "$dry" = 1 ]; then
  echo "dry run: skipping npm publish"
  exit 0
fi
npm publish --access public
