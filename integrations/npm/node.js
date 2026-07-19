// Node-only helpers: the file-map plumbing done for you. `renderFile` /
// `compileFileToJs` / `analyzeFile` read the entry and its transitive
// includes from disk, build the `{name: source}` map, and call the core
// API. Import from "@fhtml/core/node"; the root export stays free of
// node:* imports so browsers and edge runtimes never see this file.
//
// Requires `await init()` first, like everything else: include discovery
// itself runs through the wasm (`analyze` reports each file's include
// lines), so path semantics can never drift from the compiler's.

import { mkdirSync, readFileSync, renameSync, unlinkSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import posix from "node:path/posix";

import { analyze, compileToJs, FhtmlError, render } from "./index.js";

/**
 * Reads `path` and every transitively included file into a `{files, entry}`
 * pair ready for `render`/`compileToJs`/`analyze`. Keys are `/`-separated
 * paths relative to the entry's directory (an include above it keeps its
 * `../`). A file that can't be read is simply left out of the map — the
 * compiler then reports the `cannot include` error at the include site;
 * only an unreadable entry throws here.
 */
export function loadFiles(path) {
  const entryAbs = resolve(path);
  const entryDir = dirname(entryAbs);
  const entry = basename(entryAbs);
  const files = {};
  const queue = [entry];
  while (queue.length) {
    const key = queue.shift();
    if (key in files) continue;
    let src;
    try {
      src = readFileSync(resolve(entryDir, key), "utf8");
    } catch (e) {
      if (key === entry) throw e;
      continue;
    }
    files[key] = src;
    for (const inc of analyze({ [key]: src }, { entry: key }).includes) {
      queue.push(includeTarget(key, inc.path));
    }
  }
  return { files, entry };
}

/** `render`, but from a file on disk — includes resolved like the CLI. */
export function renderFile(path, opts = {}) {
  const { files, entry } = loadFiles(path);
  return render(files, { ...opts, entry });
}

/** `compileToJs`, but from a file on disk. */
export function compileFileToJs(path, opts = {}) {
  const { files, entry } = loadFiles(path);
  return compileToJs(files, { ...opts, entry });
}

/** `analyze`, but from a file on disk. */
export function analyzeFile(path) {
  const { files, entry } = loadFiles(path);
  return analyze(files, { entry });
}

/** The file an `include <path>` line in `fromKey` names, in key space —
 * mirrors the compiler exactly (SPEC §10.5): leading `./`s dropped,
 * `.fhtml` appended if absent, relative to the including file's directory,
 * then normalized lexically the way the in-memory VFS unifies spellings. */
function includeTarget(fromKey, incPath) {
  let bare = incPath;
  while (bare.startsWith("./")) bare = bare.slice(2);
  if (!bare.endsWith(".fhtml")) bare += ".fhtml";
  return posix.normalize(posix.join(posix.dirname(fromKey), bare));
}

// ---- compileFilesToDir ----------------------------------------------------

const MANIFEST = ".fhtml-manifest.json";
const MANIFEST_VERSION = 1;
const PKG_JSON = "package.json";
// Emitted modules are ESM; this scope file makes the output directory
// self-describing even inside a CommonJS project.
const PKG_JSON_CONTENT = '{\n  "type": "module"\n}\n';
const NAME_RE = /^[A-Za-z0-9_][A-Za-z0-9._-]*$/;
// Windows device basenames can't reliably become files; the check ignores
// extensions, like Windows does.
const WIN_DEVICES = /^(con|prn|aux|nul|com[1-9]|lpt[1-9])$/i;
const RENDER_FN_DTS = "(data?: unknown, ctx?: unknown) => string";
const VIEW_DTS = `declare const render: ${RENDER_FN_DTS};\nexport default render;\n`;
const TMP_RETRIES = 5;

/**
 * Compiles a set of `.fhtml` entries into `outDir` as ES modules plus a
 * registry index, safely for a live dev loop. Guarantees: the directory
 * is never wiped; every file lands via temp-file + `rename()` so readers
 * never see a half-written file; `index.js` (the completion signal) is
 * swapped last, after everything it imports exists; pruning removes only
 * files this helper previously emitted (tracked in `.fhtml-manifest.json`)
 * and runs after the fresh index is live; unchanged outputs are skipped,
 * so watchers of `outDir` don't loop. A compile error throws before any
 * write. Scope: one writer per `outDir` at a time; per-file atomicity on
 * ordinary local filesystems (no fsync — not a durability guarantee);
 * a reader still holding an older index is not protected from pruning.
 *
 * `entries` is an array of `.fhtml` paths (view name = filename stem) or
 * a `{name: path}` map. Requires `await init()` first.
 */
export function compileFilesToDir(opts) {
  const { entries, outDir, mode, emitIndex = true, emitDts = true, prune = true } = opts;
  const views = normalizeEntries(entries, emitIndex);

  // Compile everything before the first write: a broken view leaves the
  // output directory byte-for-byte untouched.
  const warnings = [];
  const compiled = views.map(({ name, path }) => {
    let out;
    try {
      out = compileFileToJs(path, { mode });
    } catch (e) {
      if (e instanceof FhtmlError) e.file = path;
      throw e;
    }
    for (const w of out.warnings) warnings.push({ file: path, msg: w.msg });
    return { name, js: out.js };
  });

  const outputs = new Map(); // filename -> content, in write order
  outputs.set(PKG_JSON, PKG_JSON_CONTENT); // the module scope goes in first
  for (const { name, js } of compiled) {
    outputs.set(`${name}.js`, js);
    if (emitDts) outputs.set(`${name}.d.ts`, VIEW_DTS);
  }
  if (emitIndex) {
    const names = compiled.map((c) => c.name);
    outputs.set("index.d.ts", indexDts(names));
    outputs.set("index.js", indexJs(names)); // the completion signal, last
  }

  const prev = readManifest(outDir);
  mkdirSync(outDir, { recursive: true });

  const written = [];
  const unchanged = [];
  for (const [file, content] of outputs) {
    const changed = writeAtomic(join(outDir, file), content);
    if (file === PKG_JSON) continue; // internal, like the manifest
    (changed ? written : unchanged).push(file);
  }

  // Prune only after the fresh index (which no longer references removed
  // views) is live. A file that can't be deleted stays owned, so a later
  // run retries it; the manifest is written last so a failed deletion is
  // never forgotten.
  const current = new Set(outputs.keys());
  const pruned = [];
  const owned = new Set(current);
  for (const file of prev) {
    if (current.has(file)) continue;
    if (!prune) {
      owned.add(file);
      continue;
    }
    try {
      unlinkSync(join(outDir, file));
      pruned.push(file);
    } catch (e) {
      if (e.code === "ENOENT") continue; // already gone: pruned enough
      owned.add(file);
    }
  }
  writeAtomic(
    join(outDir, MANIFEST),
    JSON.stringify({ version: MANIFEST_VERSION, files: [...owned].sort() }, null, 2) + "\n",
  );

  return { written, unchanged, pruned: pruned.sort(), warnings };
}

/** `entries` → sorted `[{name, path}]`, every name validated as a safe,
 * portable output stem and checked for case-folded collisions. */
function normalizeEntries(entries, emitIndex) {
  const pairs = Array.isArray(entries)
    ? entries.map((p) => {
        const b = basename(p);
        return [b.endsWith(".fhtml") ? b.slice(0, -".fhtml".length) : b, p];
      })
    : Object.entries(entries);
  const seen = new Map(); // case-folded name -> original
  for (const [name, path] of pairs) {
    if (!NAME_RE.test(name)) {
      throw new Error(
        `fhtml: invalid view name ${JSON.stringify(name)} (for ${path}) — ` +
          `names become filenames: a letter/digit/_ then letters/digits/._- only`,
      );
    }
    if (WIN_DEVICES.test(name.split(".")[0])) {
      throw new Error(`fhtml: view name ${JSON.stringify(name)} is a Windows device name`);
    }
    if (emitIndex && name.toLowerCase() === "index") {
      throw new Error(
        'fhtml: view name "index" collides with the registry module — rename it or pass emitIndex: false',
      );
    }
    const folded = name.toLowerCase();
    if (seen.has(folded)) {
      throw new Error(
        `fhtml: view names ${JSON.stringify(seen.get(folded))} and ${JSON.stringify(name)} ` +
          `collide on case-insensitive filesystems — use the {name: path} form`,
      );
    }
    seen.set(folded, name);
  }
  return pairs.map(([name, path]) => ({ name, path })).sort((a, b) => (a.name < b.name ? -1 : 1));
}

// Computed keys throughout: a literal `"__proto__": $0` in an object
// initializer would set the prototype instead of defining the view.
function indexJs(names) {
  const imports = names.map((n, i) => `import $${i} from ${JSON.stringify(`./${n}.js`)};`);
  const keys = names.map((n, i) => `  [${JSON.stringify(n)}]: $${i},`);
  const head = imports.length ? imports.join("\n") + "\n\n" : "";
  return `${head}export const views = {\n${keys.join("\n")}\n};\nexport default views;\n`;
}

function indexDts(names) {
  const keys = names.map((n) => `  ${JSON.stringify(n)}: RenderFn;`);
  return (
    `export type RenderFn = ${RENDER_FN_DTS};\n` +
    `export declare const views: {\n${keys.join("\n")}\n};\n` +
    `export default views;\n`
  );
}

/** The previous run's output list. Fails closed: anything unexpected —
 * unreadable, bad JSON, wrong version, absolute paths, separators, `..`,
 * the manifest naming itself — yields `[]`, i.e. prune nothing. */
function readManifest(outDir) {
  let m;
  try {
    m = JSON.parse(readFileSync(join(outDir, MANIFEST), "utf8"));
  } catch {
    return [];
  }
  if (!m || m.version !== MANIFEST_VERSION || !Array.isArray(m.files)) return [];
  const ok = m.files.every(
    (f) => typeof f === "string" && f !== MANIFEST && NAME_RE.test(f),
  );
  return ok ? m.files : [];
}

let tmpSeq = 0;

/** Write-then-rename, so a reader never observes a missing or partial
 * file; identical content skips the swap (and the watcher event). The
 * temp file lives next to the target — `rename()` is only atomic within
 * one filesystem. Returns whether the target changed. */
function writeAtomic(target, content) {
  try {
    if (readFileSync(target, "utf8") === content) return false;
  } catch {
    // unreadable/absent/directory: fall through and let the write decide
  }
  for (let tries = 0; ; tries++) {
    const tmp = `${target}.${process.pid}.${tmpSeq++}.tmp`;
    try {
      writeFileSync(tmp, content, { flag: "wx" }); // exclusive: no PID-reuse litter races
    } catch (e) {
      if (e.code === "EEXIST" && tries < TMP_RETRIES) continue;
      throw e;
    }
    try {
      renameSync(tmp, target);
    } catch (e) {
      try {
        unlinkSync(tmp);
      } catch {
        // the rename error is the one worth reporting
      }
      throw e;
    }
    return true;
  }
}
