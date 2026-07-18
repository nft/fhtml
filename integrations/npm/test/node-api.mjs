// Tests for the "@fhtml/core/node" subpath: loadFiles builds the include
// closure from disk, and renderFile/compileFileToJs stay byte-identical
// to the native CLI on a real multi-file tree (nested includes, a `..`
// include, defs joining one namespace). Run via test.sh: needs FHTML_BIN
// and ../fhtml.wasm.

import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { join } from "node:path";
import assert from "node:assert/strict";

import { init, FhtmlError } from "../index.js";
import { analyzeFile, compileFileToJs, loadFiles, renderFile } from "../node.js";

const bin = process.env.FHTML_BIN;
assert.ok(bin, "FHTML_BIN must point at the native CLI");

await init();

const root = mkdtempSync(join(tmpdir(), "fhtml-node-api-"));
try {
  mkdirSync(join(root, "views/partials/deep"), { recursive: true });
  const write = (rel, src) => writeFileSync(join(root, rel), src);
  write(
    "views/main.fhtml",
    'include ./partials/lib\n\ndiv grid\n  +card(title="Hi")\n    p "body"\n  +brand()\n',
  );
  write(
    "views/partials/lib.fhtml",
    'include ./deep/head\n\ndef card(title)\n  . rounded\n    h3 "{title}"\n    children\n',
  );
  // `../shared` exercises the `..` path in key space.
  write("views/partials/deep/head.fhtml", "include ../shared\n");
  write("views/partials/shared.fhtml", 'def brand()\n  span "B"\n');

  const entry = join(root, "views/main.fhtml");

  // loadFiles: the exact closure, keyed relative to the entry's directory.
  const { files, entry: entryKey } = loadFiles(entry);
  assert.equal(entryKey, "main.fhtml");
  assert.deepEqual(Object.keys(files).sort(), [
    "main.fhtml",
    "partials/deep/head.fhtml",
    "partials/lib.fhtml",
    "partials/shared.fhtml",
  ]);

  // Byte-parity with the native CLI on the same tree, both modes + js.
  const native = (args) => {
    const r = spawnSync(bin, args, { encoding: "utf8" });
    assert.equal(r.status, 0, `${bin} ${args.join(" ")}: ${r.stderr}`);
    return r.stdout;
  };
  assert.equal(renderFile(entry).html, native([entry]));
  assert.equal(
    renderFile(entry, { mode: "pretty" }).html,
    native(["--pretty", entry]),
  );
  assert.equal(compileFileToJs(entry).js, native(["--target=js", entry]));

  // analyzeFile sees cross-file defs with their map keys.
  const a = analyzeFile(entry);
  assert.equal(a.error, null);
  assert.equal(a.defs.find((d) => d.name === "card").file, "partials/lib.fhtml");
  assert.equal(
    a.defs.find((d) => d.name === "brand").file,
    "partials/shared.fhtml",
  );

  // A missing include is the compiler's error, at the include site.
  write("views/broken.fhtml", "include ./nope\n");
  try {
    renderFile(join(root, "views/broken.fhtml"));
    assert.fail("should have thrown");
  } catch (e) {
    assert.ok(e instanceof FhtmlError);
    assert.equal(e.line, 1);
    assert.match(e.message, /cannot include/);
  }

  // A missing entry throws the fs error, not a compiler one.
  assert.throws(() => renderFile(join(root, "views/absent.fhtml")), /ENOENT/);
} finally {
  rmSync(root, { recursive: true, force: true });
}

console.log("node-api.mjs: all assertions passed");
