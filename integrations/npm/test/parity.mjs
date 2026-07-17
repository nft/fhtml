// The gate: over every corpus file,
// wasm output is byte-identical to the native CLI — render (min and
// pretty) vs `fhtml`, compileToJs vs `fhtml --target=js`, format vs
// `fhtml fmt`. Initializes via the default URL loader (api.mjs covers
// the bytes override). Run via test.sh: needs FHTML_BIN and ../fhtml.wasm.

import { readFileSync, readdirSync, existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { basename, join } from "node:path";
import assert from "node:assert/strict";

import { init, render, compileToJs, format } from "../index.js";

const bin = process.env.FHTML_BIN;
assert.ok(bin, "FHTML_BIN must point at the native CLI");
const repo = new URL("../../..", import.meta.url).pathname;

await init(); // the default loader: new URL("fhtml.wasm", import.meta.url)

function native(args, input) {
  const r = spawnSync(bin, args, { input, encoding: "utf8", maxBuffer: 64 << 20 });
  assert.equal(r.status, 0, `${bin} ${args.join(" ")}: ${r.stderr}`);
  return r.stdout;
}

const corpus = [];
const benchDir = join(repo, "bench/out/fhtml");
if (existsSync(benchDir)) {
  for (const f of readdirSync(benchDir).sort()) {
    if (f.endsWith(".fhtml")) corpus.push(join(benchDir, f));
  }
} else {
  console.log("note: bench/out/fhtml not present — corpus is site/ only");
}
for (const f of readdirSync(join(repo, "site")).sort()) {
  if (f.endsWith(".fhtml")) corpus.push(join(repo, "site", f));
}
if (existsSync(benchDir)) {
  assert.ok(corpus.length >= 49, `expected the full corpus, found ${corpus.length}`);
}

let checks = 0;
for (const path of corpus) {
  const src = readFileSync(path, "utf8");
  const name = basename(path);
  const files = { [name]: src };

  // render, min and pretty — stdout is the exact html (print!, no newline)
  assert.equal(render(files, { mode: "min" }).html, native([path]), `${name}: min`);
  assert.equal(
    render(files, { mode: "pretty" }).html,
    native(["--pretty", path]),
    `${name}: pretty`,
  );

  // the JS target
  assert.equal(
    compileToJs(files, { mode: "min" }).js,
    native(["--target=js", path]),
    `${name}: js`,
  );

  // fmt via stdin prints to stdout
  assert.equal(format(src), native(["fmt", "-"], src), `${name}: fmt`);

  checks += 4;
}

console.log(
  `parity.mjs: ${corpus.length} corpus files, ${checks} byte-identical outputs vs the native CLI`,
);
