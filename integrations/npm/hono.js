// Hono renderer middleware: `app.use(fhtmlRenderer(templates))`, then
// `c.render("page", data)` in handlers. Hono's main habitat is edge
// runtimes with no filesystem, so templates come in as the usual
// `{name: source}` map (bundle them at build time) and the wasm can be
// passed in for platforms where the default loader can't reach it
// (Workers: `import wasm from "@fhtml/core/fhtml.wasm"`). hono itself is
// never imported — the middleware is just `(c, next)` — so browser and
// edge bundles carry exactly the core and nothing else.

import { init, render } from "./index.js";

const EXT = ".fhtml";

/**
 * Returns a Hono middleware that installs an fhtml renderer:
 * `c.render(name, data, ctx?)` renders `files[name]` (the `.fhtml`
 * extension may be omitted, like an `include` path) and responds with
 * `c.html(...)`. A compile/render error throws `FhtmlError` into Hono's
 * normal `onError` path.
 */
export function fhtmlRenderer(files, opts = {}) {
  return async (c, next) => {
    await init(opts.wasm); // idempotent; lazy so module load stays sync-safe
    c.setRenderer((name, data, ctx) => {
      const entry = name.endsWith(EXT) ? name : name + EXT;
      const { html, warnings } = render(files, {
        entry,
        data,
        ctx: ctx ?? opts.ctx,
        mode: opts.mode,
      });
      if (warnings.length) opts.onWarnings?.(warnings, entry);
      return c.html(html);
    });
    await next();
  };
}
