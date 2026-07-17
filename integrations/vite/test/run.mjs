// Gate assertions for vite-plugin-fhtml.
// Driven by test.sh, which builds the compiler and sets FHTML_BIN.
//
// Fixtures are copied to .work/ so the HMR test can edit a partial without
// touching the committed files (.work/ also keeps every path on a real,
// symlink-free prefix — `fhtml deps` prints canonical paths, and the watcher
// must see the same ones).

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

import { build, createLogger, createServer } from "vite";
import fhtml from "vite-plugin-fhtml";

const FHTML = process.env.FHTML_BIN || "fhtml";
const here = path.dirname(fileURLToPath(import.meta.url));
const work = path.join(here, ".work");

const cli = (args) => execFileSync(FHTML, args, { encoding: "utf8" });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
let passed = 0;
const ok = (name) => {
  passed += 1;
  console.log(`ok - ${name}`);
};

fs.rmSync(work, { recursive: true, force: true });
fs.cpSync(path.join(here, "src"), work, { recursive: true });

// ---- build: default import + ?html, byte-equal to the CLI ---------------

await build({
  root: work,
  configFile: false,
  logLevel: "error",
  plugins: [fhtml()],
  build: {
    lib: { entry: path.join(work, "entry.js"), formats: ["es"], fileName: "entry" },
    outDir: path.join(work, "dist"),
    minify: false,
  },
});
const bundle = await import(pathToFileURL(path.join(work, "dist/entry.js")));
const data = JSON.parse(fs.readFileSync(path.join(work, "data.json"), "utf8"));

assert.equal(
  bundle.renderCard(data),
  cli(["--min", "--data", path.join(work, "data.json"), path.join(work, "card.fhtml")])
);
ok("build: render function output is byte-equal to `fhtml --min --data`");

assert.equal(bundle.heroHtml, cli(["--min", path.join(work, "hero.fhtml")]));
ok("build: ?html import is byte-equal to `fhtml --min`");

// ---- dev server: HMR across includes, diagnostics -----------------------

const warns = [];
const logger = createLogger("info", { allowClearScreen: false });
logger.warn = (msg) => warns.push(msg);

const server = await createServer({
  root: work,
  configFile: false,
  logLevel: "error",
  customLogger: logger,
  plugins: [fhtml()],
  // hmr stays on: handleHotUpdate (the include-invalidation path under
  // test) only runs inside the HMR pipeline.
  server: { middlewareMode: true },
});

try {
  const first = await server.transformRequest("/card.fhtml");
  assert.ok(first.code.includes("©"), "include content reaches the module");
  assert.ok(!first.code.includes("EDITED-MARKER"));

  // Editing a transitively included partial must invalidate the importer:
  // poll transformRequest — it returns the cached module until the change
  // event reaches handleHotUpdate. Let the watcher finish its initial scan
  // first, or the edit lands unseen (the race is the test's, not the
  // plugin's: real edits come long after startup).
  await Promise.race([
    new Promise((resolve) => server.watcher.once("ready", resolve)),
    sleep(2000),
  ]);
  fs.appendFileSync(path.join(work, "partials/footer.fhtml"), 'p "EDITED-MARKER"\n');
  let recompiled;
  for (let i = 0; i < 100; i++) {
    recompiled = await server.transformRequest("/card.fhtml");
    if (recompiled.code.includes("EDITED-MARKER")) break;
    await sleep(100);
  }
  assert.ok(
    recompiled.code.includes("EDITED-MARKER"),
    "editing an included partial must rebuild the importer within 10s"
  );
  ok("dev: editing an included partial invalidates the importing module");

  // A syntax error carries file:line:col into the overlay payload.
  fs.writeFileSync(path.join(work, "bad.fhtml"), 'p "unclosed\n');
  const err = await server.transformRequest("/bad.fhtml").then(
    () => assert.fail("bad.fhtml must not compile"),
    (e) => e
  );
  assert.equal(err.loc?.line, 1);
  assert.equal(err.loc?.column, 12);
  assert.ok(err.loc?.file.endsWith("bad.fhtml"), `got file: ${err.loc?.file}`);
  assert.ok(err.message.includes("unclosed string"), `got: ${err.message}`);
  ok("dev: compile error surfaces with file:line:col loc");

  // ?html on a templated file is the compiler's own error, verbatim.
  const htmlErr = await server.transformRequest("/card.fhtml?html").then(
    () => assert.fail("templated ?html import must fail"),
    (e) => e
  );
  assert.ok(htmlErr.message.includes("template construct"), `got: ${htmlErr.message}`);
  ok("dev: ?html on a templated file fails with the compiler's error");

  // Compiler warnings (the §9.1 concat-class lint) pass through this.warn.
  await server.transformRequest("/warn.fhtml");
  assert.ok(
    warns.some((w) => w.includes("string concatenation")),
    `got warns: ${JSON.stringify(warns)}`
  );
  ok("dev: compiler warnings reach the Vite logger");
} finally {
  await server.close();
}

// ---- binary resolution: missing binary fails with the install hint ------

const missing = fhtml({ bin: path.join(work, "no-such-fhtml") });
const thrown = (() => {
  try {
    missing.load.call(
      { error: (e) => assert.fail(`reached this.error: ${e}`) },
      path.join(work, "card.fhtml")
    );
    return null;
  } catch (e) {
    return e;
  }
})();
assert.ok(thrown?.message.includes("cargo install"), `got: ${thrown?.message}`);
ok("missing fhtml binary fails with the install hint");

fs.rmSync(work, { recursive: true, force: true });
console.log(`\nvite-plugin-fhtml: ${passed} check(s) passed`);
