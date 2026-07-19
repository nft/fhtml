// compileFilesToDir: the batch/dir compile helper and its correctness
// guarantees — write ordering, atomicity, manifest-tracked pruning,
// naming validation, and the generated registry's edge cases.
import assert from "node:assert/strict";
import {
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmdirSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";

import { FhtmlError, init } from "../index.js";
import { compileFilesToDir, renderFile } from "../node.js";

await init();

const MANIFEST = ".fhtml-manifest.json";

const root = mkdtempSync(join(tmpdir(), "fhtml-dir-api-"));
const views = join(root, "views");
mkdirSync(views);
writeFileSync(join(views, "_lib.fhtml"), 'def badge(label)\n  span rounded "{label}"\n');
writeFileSync(join(views, "card.fhtml"), "include ./_lib\n\n. p-4\n  +badge(label={name})\n");
writeFileSync(join(views, "page.fhtml"), 'main "Hello, {name}"\n');

// The importing project is CommonJS on purpose: the emitted package.json
// must make outDir an ESM scope on its own (Node 18 semantics).
writeFileSync(join(root, "package.json"), '{ "type": "commonjs" }\n');
const out = join(root, "generated");

const snapshot = (dir) =>
  Object.fromEntries(
    readdirSync(dir)
      .sort()
      .map((f) => [f, readFileSync(join(dir, f), "utf8")]),
  );

// ---- end-to-end: compile, import the registry, render ---------------------

{
  const entries = [join(views, "card.fhtml"), join(views, "page.fhtml")];
  const res = compileFilesToDir({ entries, outDir: out });
  assert.deepEqual(res.written, ["card.js", "card.d.ts", "page.js", "page.d.ts", "index.d.ts", "index.js"]);
  assert.deepEqual(res.unchanged, []);
  assert.deepEqual(res.pruned, []);
  assert.deepEqual(
    Object.keys(snapshot(out)).sort(),
    [MANIFEST, "card.d.ts", "card.js", "index.d.ts", "index.js", "package.json", "page.d.ts", "page.js"],
  );

  const { views: registry } = await import(pathToFileURL(join(out, "index.js")));
  const data = { name: "hi" };
  assert.equal(registry.card(data), renderFile(join(views, "card.fhtml"), { data }).html);
  assert.equal(registry.page(data), renderFile(join(views, "page.fhtml"), { data }).html);

  const manifest = JSON.parse(readFileSync(join(out, MANIFEST), "utf8"));
  assert.equal(manifest.version, 1);
  assert.ok(manifest.files.includes("package.json"));
}

// ---- determinism: a re-run swaps nothing ----------------------------------

{
  const before = snapshot(out);
  const res = compileFilesToDir({
    entries: [join(views, "card.fhtml"), join(views, "page.fhtml")],
    outDir: out,
  });
  assert.deepEqual(res.written, []);
  assert.equal(res.unchanged.length, 6);
  assert.deepEqual(snapshot(out), before);
}

// ---- compile failure: throws with .file, before any write -----------------

{
  writeFileSync(join(views, "bad.fhtml"), 'span "unclosed\n');
  const before = snapshot(out);
  assert.throws(
    () =>
      compileFilesToDir({
        entries: [join(views, "card.fhtml"), join(views, "bad.fhtml")],
        outDir: out,
      }),
    (e) => e instanceof FhtmlError && e.file === join(views, "bad.fhtml") && e.line === 1,
  );
  assert.deepEqual(snapshot(out), before, "a compile error must leave outDir untouched");
}

// ---- rename failure: error surfaces, no tmp litter ------------------------

{
  const blockedOut = join(root, "blocked");
  mkdirSync(join(blockedOut, "card.js"), { recursive: true });
  writeFileSync(join(blockedOut, "card.js", "occupant"), "x");
  assert.throws(() =>
    compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: blockedOut }),
  );
  assert.ok(
    !readdirSync(blockedOut).some((f) => f.endsWith(".tmp")),
    "failed writes must clean up their temp files",
  );
}

// ---- prune: removed view's outputs go, strays survive ---------------------

{
  writeFileSync(join(out, "stray.txt"), "hand-written");
  const res = compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: out });
  assert.deepEqual(res.pruned, ["page.d.ts", "page.js"]);
  const files = Object.keys(snapshot(out));
  assert.ok(!files.includes("page.js") && files.includes("stray.txt"));
  assert.ok(!readFileSync(join(out, "index.js"), "utf8").includes("page"));
}

// ---- prune: false retains ownership of leftovers --------------------------

{
  const dir = join(root, "prune-false");
  const both = [join(views, "card.fhtml"), join(views, "page.fhtml")];
  compileFilesToDir({ entries: both, outDir: dir });
  const res = compileFilesToDir({ entries: [both[0]], outDir: dir, prune: false });
  assert.deepEqual(res.pruned, []);
  assert.ok(Object.keys(snapshot(dir)).includes("page.js"), "prune: false keeps the file");
  const manifest = JSON.parse(readFileSync(join(dir, MANIFEST), "utf8"));
  assert.ok(manifest.files.includes("page.js"), "…and keeps owning it");
  const res2 = compileFilesToDir({ entries: [both[0]], outDir: dir });
  assert.deepEqual(res2.pruned, ["page.d.ts", "page.js"], "a later prune: true still works");
}

// ---- prune failure: file stays owned, retried next run --------------------

{
  const dir = join(root, "prune-retry");
  compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: dir });
  const manifest = JSON.parse(readFileSync(join(dir, MANIFEST), "utf8"));
  manifest.files.push("ghost.js");
  writeFileSync(join(dir, MANIFEST), JSON.stringify(manifest) + "\n");
  mkdirSync(join(dir, "ghost.js")); // unlink() on a directory fails
  const res = compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: dir });
  assert.deepEqual(res.pruned, []);
  assert.ok(
    JSON.parse(readFileSync(join(dir, MANIFEST), "utf8")).files.includes("ghost.js"),
    "an undeletable file must stay owned",
  );
  rmdirSync(join(dir, "ghost.js"));
  writeFileSync(join(dir, "ghost.js"), "now a file");
  const res2 = compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: dir });
  assert.deepEqual(res2.pruned, ["ghost.js"]);
}

// ---- manifests fail closed ------------------------------------------------

{
  const dir = join(root, "bad-manifest");
  compileFilesToDir({ entries: [join(views, "page.fhtml")], outDir: dir });
  writeFileSync(join(dir, "orphan.js"), "leftover");
  for (const bad of [
    "not json",
    '{ "version": 99, "files": ["orphan.js"] }',
    '{ "version": 1, "files": ["../escape.js"] }',
    '{ "version": 1, "files": ["sub/dir.js"] }',
    `{ "version": 1, "files": ["${MANIFEST}"] }`,
  ]) {
    writeFileSync(join(dir, MANIFEST), bad);
    const res = compileFilesToDir({ entries: [join(views, "page.fhtml")], outDir: dir });
    assert.deepEqual(res.pruned, [], `must prune nothing for manifest: ${bad}`);
  }
  assert.ok(Object.keys(snapshot(dir)).includes("orphan.js"));
}

// ---- naming: collisions, reservations, portability ------------------------

{
  const dupA = join(root, "dup-a");
  const dupB = join(root, "dup-b");
  mkdirSync(dupA);
  mkdirSync(dupB);
  writeFileSync(join(dupA, "card.fhtml"), 'p "a"\n');
  writeFileSync(join(dupB, "card.fhtml"), 'p "b"\n');
  const dir = join(root, "naming-out");
  assert.throws(
    () => compileFilesToDir({ entries: [join(dupA, "card.fhtml"), join(dupB, "card.fhtml")], outDir: dir }),
    /collide/,
  );
  // …which the map form resolves
  compileFilesToDir({
    entries: { "card-a": join(dupA, "card.fhtml"), "card-b": join(dupB, "card.fhtml") },
    outDir: dir,
  });

  const page = join(views, "page.fhtml");
  assert.throws(() => compileFilesToDir({ entries: { "no/sep": page }, outDir: dir }), /invalid view name/);
  assert.throws(() => compileFilesToDir({ entries: { ".dot": page }, outDir: dir }), /invalid view name/);
  assert.throws(() => compileFilesToDir({ entries: { CON: page }, outDir: dir }), /device name/);
  assert.throws(() => compileFilesToDir({ entries: { "nul.view": page }, outDir: dir }), /device name/);
  assert.throws(
    () => compileFilesToDir({ entries: { Card: page, card: page }, outDir: dir }),
    /case-insensitive/,
  );
  assert.throws(() => compileFilesToDir({ entries: { index: page }, outDir: dir }), /registry/);
  compileFilesToDir({ entries: { index: page }, outDir: join(root, "no-index"), emitIndex: false });
}

// ---- the registry survives __proto__ --------------------------------------

{
  const dir = join(root, "proto");
  // computed key here too — a literal `__proto__:` would set the test
  // object's prototype, which is the whole point of this test
  compileFilesToDir({ entries: { ["__proto__"]: join(views, "page.fhtml") }, outDir: dir });
  const mod = await import(pathToFileURL(join(dir, "index.js")));
  const desc = Object.getOwnPropertyDescriptor(mod.views, "__proto__");
  assert.ok(desc && typeof desc.value === "function", "__proto__ must be an own property");
  assert.equal(mod.views["__proto__"]({ name: "p" }), renderFile(join(views, "page.fhtml"), { data: { name: "p" } }).html);
}

// ---- empty entries, emit flags --------------------------------------------

{
  const dir = join(root, "empty");
  compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: dir });
  const res = compileFilesToDir({ entries: [], outDir: dir });
  assert.deepEqual(res.pruned, ["card.d.ts", "card.js"]);
  const mod = await import(pathToFileURL(join(dir, "index.js")));
  assert.deepEqual(Object.keys(mod.views), []);

  const noDts = join(root, "no-dts");
  compileFilesToDir({ entries: [join(views, "card.fhtml")], outDir: noDts, emitDts: false });
  const files = Object.keys(snapshot(noDts));
  assert.ok(!files.includes("card.d.ts"), "emitDts: false skips view shims");
  assert.ok(files.includes("index.d.ts"), "…but index.d.ts belongs to emitIndex");

  const bare = join(root, "bare");
  const res2 = compileFilesToDir({
    entries: [join(views, "card.fhtml")],
    outDir: bare,
    emitIndex: false,
    emitDts: false,
  });
  assert.deepEqual(res2.written, ["card.js"]);
}

// ---- warnings come back flattened, attributed to the entry ----------------

{
  writeFileSync(join(views, "warny.fhtml"), 'div\n  div\n      span "x"\n');
  const res = compileFilesToDir({ entries: [join(views, "warny.fhtml")], outDir: join(root, "warn-out") });
  assert.equal(res.warnings.length, 1);
  assert.equal(res.warnings[0].file, join(views, "warny.fhtml"));
  assert.match(res.warnings[0].msg, /indent step/);
}

rmSync(root, { recursive: true, force: true });
console.log("dir-api.mjs: all assertions passed");
