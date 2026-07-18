// Glue API tests: the public surface
// of index.js. Initializes via the `init(bytes)` override — the
// Workers-style path; parity.mjs covers the default URL loader.
// Run via test.sh (needs ../fhtml.wasm built and copied).

import { readFile } from "node:fs/promises";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import assert from "node:assert/strict";

import {
  init,
  render,
  compileToJs,
  format,
  analyze,
  version,
  FhtmlError,
} from "../index.js";

// ---- before init: a clear error, not a crash ------------------------------

assert.throws(() => render("div\n"), /await init/);

// ---- init via bytes (Workers-style), idempotent ---------------------------

const bytes = await readFile(new URL("../fhtml.wasm", import.meta.url));
await init(bytes);
await init(); // second call is a no-op, with or without an argument

assert.match(version(), /^\d+\.\d+\.\d+/);

// ---- single-string render -------------------------------------------------

{
  const { html, warnings } = render('div grid\n  span rounded "hi"\n');
  assert.equal(html, '<div class="grid"><span class="rounded">hi</span></div>');
  assert.deepEqual(warnings, []);
}

// ---- file map + entry + data/ctx + pretty ---------------------------------

const files = {
  "lib.fhtml": 'def badge(label)\n  span rounded "{label}"\n',
  "main.fhtml": 'include ./lib\n\ndiv grid\n  +badge(label={name})\n  p "{ctx.who}"\n',
};

{
  const { html } = render(files, {
    entry: "main.fhtml",
    data: { name: "hi" },
    ctx: { who: "us" },
  });
  assert.equal(
    html,
    '<div class="grid"><span class="rounded">hi</span><p>us</p></div>',
  );
  const pretty = render(files, { entry: "main.fhtml", mode: "pretty" });
  assert.match(pretty.html, /\n/);
}

// ---- backslashed keys normalize (Windows-host hygiene) --------------------

{
  const { html } = render(
    {
      "dir\\lib.fhtml": 'def badge()\n  span "b"\n',
      "main.fhtml": "include ./dir/lib\n\n+badge()\n",
    },
    { entry: "main.fhtml" },
  );
  assert.equal(html, '<span>b</span>');
}

// ---- compileToJs: the emitted module actually runs ------------------------

{
  const { js } = compileToJs(files, { entry: "main.fhtml" });
  assert.match(js, /export default/);
  // Imported from a real file — the production path, and it works on
  // Node and Bun alike (Bun 1.3 cannot import data: URLs).
  const dir = mkdtempSync(join(tmpdir(), "fhtml-api-"));
  try {
    const out = join(dir, "emitted.mjs");
    writeFileSync(out, js);
    const mod = await import(pathToFileURL(out).href);
    assert.equal(
      mod.default({ name: "hi" }, { who: "us" }),
      '<div class="grid"><span class="rounded">hi</span><p>us</p></div>',
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// ---- format ---------------------------------------------------------------

// Canonical form: 2-space indent, `.` for div.
assert.equal(format('div\n    span   "hi"\n'), '.\n  span "hi"\n');
assert.equal(
  format('#!shorthand\n. fx\n', { shorthand: "expand" }),
  '. flex\n',
);

// ---- analyze --------------------------------------------------------------

{
  const a = analyze(files, { entry: "main.fhtml" });
  assert.equal(a.error, null);
  const badge = a.defs.find((d) => d.name === "badge");
  assert.equal(badge.file, "lib.fhtml");
  assert.deepEqual(badge.nameSpan, { line: 1, col: 5, len: 5 });
  assert.equal(badge.params[0].name, "label");
  assert.equal(a.includes[0].resolved, "lib.fhtml");
  assert.equal(a.calls.find((c) => c.name === "badge").args[0].name, "label");

  // Broken source: never throws — the error is data on the analysis.
  const broken = analyze('span "unclosed\n');
  assert.equal(broken.error.line, 1);
  assert.match(broken.error.msg, /unclosed string/);
}

// ---- errors are FhtmlError with position data -----------------------------

{
  try {
    render('span "unclosed\n');
    assert.fail("should have thrown");
  } catch (e) {
    assert.ok(e instanceof FhtmlError);
    assert.ok(e instanceof Error);
    assert.equal(e.line, 1);
    assert.match(e.message, /unclosed string/);
  }
  try {
    render(files, { entry: "nope.fhtml" });
    assert.fail("should have thrown");
  } catch (e) {
    assert.ok(e instanceof FhtmlError);
    assert.equal(e.line, null); // not a source position
    assert.match(e.message, /not in the file map/);
  }
}

console.log("api.mjs: all assertions passed");
