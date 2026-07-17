// vite-plugin-fhtml — import .fhtml files as render functions (or, with
// `?html`, as static HTML strings). The plugin is a thin shell around the
// fhtml CLI: compilation, byte-for-byte output parity with the native
// renderer, and dependency listing all live in the compiler; nothing is
// reimplemented here.

import { spawnSync } from "node:child_process";

const NAME = "vite-plugin-fhtml";

/**
 * @typedef {object} FhtmlOptions
 * @property {string} [bin] Path to the fhtml binary. Resolution order:
 *   this option, then the FHTML_BIN environment variable, then `fhtml`
 *   on $PATH.
 */

/**
 * Vite plugin for fhtml (Fluid HTML — see SPEC.md in the fhtml repo).
 *
 * - `import render from "./card.fhtml"` — a self-contained ES module
 *   exporting `(data, ctx = {}) => string`, emitted by `fhtml --target=js`.
 *   Static files get the same shape (a render function that ignores its
 *   arguments), so imports are uniform.
 * - `import html from "./hero.fhtml?html"` — the compiled static HTML as a
 *   string (`fhtml --min`). A file that uses the template layer is a
 *   compile error under `?html` — rendering needs data.
 * - Editing a transitively `include`d partial invalidates every importer
 *   (the watch list comes from `fhtml deps`).
 *
 * @param {FhtmlOptions} [options]
 * @returns {import("vite").Plugin}
 */
export default function fhtml(options = {}) {
  const bin = options.bin || process.env.FHTML_BIN || "fhtml";

  /** include-dep → set of importing .fhtml files, rebuilt on every compile.
   * `addWatchFile` alone covers `vite build --watch`, but the dev server
   * only invalidates modules mapped to the changed file — an included
   * partial is not a module, so `handleHotUpdate` does the mapping. */
  const depImporters = new Map();

  function trackDeps(file, deps) {
    for (const importers of depImporters.values()) importers.delete(file);
    for (const dep of deps) {
      if (!depImporters.has(dep)) depImporters.set(dep, new Set());
      depImporters.get(dep).add(file);
    }
  }

  /** Run the fhtml CLI; only a missing binary throws here — compile
   * failures come back as a result for the caller to attribute. */
  function run(args) {
    const r = spawnSync(bin, args, {
      encoding: "utf8",
      maxBuffer: 64 * 1024 * 1024,
    });
    if (r.error) {
      if (r.error.code === "ENOENT") {
        throw new Error(
          `[${NAME}] fhtml binary not found (tried \`${bin}\`). Install it — ` +
            "`cargo install --path .` from the fhtml repo — or point the " +
            "plugin at one via the `bin` option or $FHTML_BIN."
        );
      }
      throw r.error;
    }
    return r;
  }

  return {
    name: NAME,

    load(id) {
      const [file, rawQuery] = id.split("?", 2);
      if (!file.endsWith(".fhtml")) return null;
      const wantsHtml = new URLSearchParams(rawQuery).has("html");

      // `--static` so a templated file under `?html` fails with the
      // compiler's "pass data" error instead of silently rendering nulls.
      const r = run(wantsHtml ? ["--static", "--min", file] : ["--target=js", file]);
      if (r.status !== 0) {
        // `file:line:col: error: msg` → a Rollup error with `loc`, so the
        // dev overlay and build output point into the .fhtml source. The
        // compiler's own wording is the whole message.
        const text = r.stderr.trim();
        const m = text.match(/^(.+?):(\d+):(\d+): error: ([\s\S]*)$/);
        this.error(
          m
            ? {
                message: m[4],
                loc: { file: m[1], line: Number(m[2]), column: Number(m[3]) },
              }
            : { message: text || `fhtml exited with status ${r.status}` }
        );
      }
      // On success stderr carries only warnings (one line each, already
      // `file:line:col: warning: …`) — e.g. the Tailwind concat-class lint.
      for (const line of r.stderr.split("\n")) {
        if (line.includes(": warning: ")) this.warn(line);
      }

      // Watch every transitively included file: editing a partial must
      // rebuild its importers. `fhtml deps` fails only where the compile
      // above already failed, so a non-zero status is unreachable here.
      const deps = run(["deps", file]);
      if (deps.status === 0) {
        const list = deps.stdout.split("\n").filter(Boolean);
        for (const dep of list) this.addWatchFile(dep);
        trackDeps(file, list);
      }

      return wantsHtml
        ? { code: `export default ${JSON.stringify(r.stdout)};`, map: null }
        : { code: r.stdout, map: null };
    },

    handleHotUpdate({ file, server, modules }) {
      const importers = depImporters.get(file);
      if (!importers) return;
      const affected = new Set(modules);
      for (const importer of importers) {
        for (const mod of server.moduleGraph.getModulesByFile(importer) ?? []) {
          affected.add(mod);
        }
      }
      return [...affected];
    },
  };
}
