// Express view engine: `app.engine("fhtml", engine())`. A thin layer over
// the node subpath — Express hands the engine a file path and a locals
// object; we load the include closure from disk and render. Import from
// "@fhtml/core/express"; express itself is never imported (the engine is
// just a callback), so there is no dependency to version-chase.
//
// `init()` runs lazily on first render, so an Express app needs no setup
// beyond registering the engine.

import { init, render } from "./index.js";
import { loadFiles } from "./node.js";

/**
 * Returns an Express view engine. Register it with
 * `app.engine("fhtml", engine())` and `app.set("view engine", "fhtml")`;
 * then `res.render("page", { name: "hi" })` renders
 * `<views>/page.fhtml` with the locals as template data.
 *
 * When Express enables view caching (`NODE_ENV=production`), the loaded
 * include closure is cached per path — no disk reads after the first
 * render of each view.
 */
export function engine(opts = {}) {
  const cache = new Map(); // path -> {files, entry}, when Express asks
  return async (path, options, cb) => {
    try {
      await init();
      // Express merges app.locals/res.locals into `options` along with
      // its own bookkeeping keys — those three aren't template data.
      const { settings, cache: useCache, _locals, ...data } = options;
      let loaded = useCache ? cache.get(path) : undefined;
      if (!loaded) {
        loaded = loadFiles(path);
        if (useCache) cache.set(path, loaded);
      }
      const { html, warnings } = render(loaded.files, {
        entry: loaded.entry,
        data,
        ctx: opts.ctx,
        mode: opts.mode,
      });
      if (warnings.length) opts.onWarnings?.(warnings, path);
      cb(null, html);
    } catch (e) {
      cb(e);
    }
  };
}
