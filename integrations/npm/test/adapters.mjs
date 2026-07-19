// The framework adapters, driven through their host contracts directly —
// no express/hono install needed, which is the point: the engine is just
// a callback, the renderer just a middleware.
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { FhtmlError, init } from "../index.js";
import { engine } from "../express.js";
import { fhtmlRenderer } from "../hono.js";

await init();

// ---- express: engine() against the (path, options, cb) contract -----------

const views = mkdtempSync(join(tmpdir(), "fhtml-adapters-"));
writeFileSync(join(views, "lib.fhtml"), 'def badge(label)\n  span rounded "{label}"\n');
writeFileSync(join(views, "page.fhtml"), "include ./lib\n\n+badge(label={name})\n");

const renderWith = (fn, path, options) =>
  new Promise((res, rej) => fn(path, options, (err, html) => (err ? rej(err) : res(html))));

// Express-shaped options bag: bookkeeping keys must not leak into data.
const page = join(views, "page.fhtml");
{
  const html = await renderWith(engine(), page, {
    settings: { views },
    _locals: {},
    name: "hi",
  });
  assert.equal(html, '<span class="rounded">hi</span>');
}

// pretty mode + static ctx via engine options
{
  writeFileSync(join(views, "ctx.fhtml"), 'p "{ctx.site}"\n');
  const html = await renderWith(engine({ ctx: { site: "fh" }, mode: "pretty" }), join(views, "ctx.fhtml"), {});
  assert.equal(html, "<p>fh</p>\n");
}

// `cache: true` (what Express sends in production) pins the include
// closure: a file edit is invisible to the same engine, visible to a
// fresh one.
{
  const cached = engine();
  await renderWith(cached, page, { cache: true, name: "a" });
  writeFileSync(join(views, "lib.fhtml"), 'def badge(label)\n  b "{label}"\n');
  const stale = await renderWith(cached, page, { cache: true, name: "a" });
  assert.equal(stale, '<span class="rounded">a</span>');
  const fresh = await renderWith(engine(), page, { cache: true, name: "a" });
  assert.equal(fresh, "<b>a</b>");
}

// errors land in the callback, not as throws
{
  await assert.rejects(() => renderWith(engine(), join(views, "missing.fhtml"), {}), /ENOENT/);
  writeFileSync(join(views, "bad.fhtml"), 'span "unclosed\n');
  await assert.rejects(
    () => renderWith(engine(), join(views, "bad.fhtml"), {}),
    (e) => e instanceof FhtmlError && e.line === 1,
  );
}

// warnings reach onWarnings (mixed indent steps warn)
{
  writeFileSync(join(views, "warn.fhtml"), 'div\n  div\n      span "x"\n');
  let seen = null;
  await renderWith(engine({ onWarnings: (w, p) => (seen = { w, p }) }), join(views, "warn.fhtml"), {});
  assert.ok(seen && seen.p.endsWith("warn.fhtml"));
  assert.match(seen.w[0].msg, /indent step/);
}

console.log("adapters (express): ok");

// ---- hono: fhtmlRenderer() against the (c, next) contract ------------------

const templates = {
  "lib.fhtml": 'def badge(label)\n  span rounded "{label}"\n',
  "page.fhtml": "include ./lib\n\n+badge(label={name})\n",
  "ctx.fhtml": 'p "{ctx.site}"\n',
};

const makeCtx = () => ({
  renderer: null,
  setRenderer(fn) {
    this.renderer = fn;
  },
  html(s) {
    return new Response(s, { headers: { "content-type": "text/html; charset=UTF-8" } });
  },
});

{
  const c = makeCtx();
  let nexted = false;
  await fhtmlRenderer(templates)(c, async () => {
    nexted = true;
  });
  assert.ok(nexted && c.renderer);

  // extension optional, like an include path
  assert.equal(await c.renderer("page", { name: "hi" }).text(), '<span class="rounded">hi</span>');
  assert.equal(await c.renderer("page.fhtml", { name: "yo" }).text(), '<span class="rounded">yo</span>');

  // errors throw into Hono's normal onError path
  assert.throws(() => c.renderer("missing"), FhtmlError);
}

// factory ctx is the default; a per-render ctx overrides it
{
  const c = makeCtx();
  await fhtmlRenderer(templates, { ctx: { site: "fh" } })(c, async () => {});
  assert.equal(await c.renderer("ctx").text(), "<p>fh</p>");
  assert.equal(await c.renderer("ctx", null, { site: "other" }).text(), "<p>other</p>");
}

console.log("adapters (hono): ok");
