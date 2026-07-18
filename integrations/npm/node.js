// Node-only helpers: the file-map plumbing done for you. `renderFile` /
// `compileFileToJs` / `analyzeFile` read the entry and its transitive
// includes from disk, build the `{name: source}` map, and call the core
// API. Import from "@fhtml/core/node"; the root export stays free of
// node:* imports so browsers and edge runtimes never see this file.
//
// Requires `await init()` first, like everything else: include discovery
// itself runs through the wasm (`analyze` reports each file's include
// lines), so path semantics can never drift from the compiler's.

import { readFileSync } from "node:fs";
import { basename, dirname, resolve } from "node:path";
import posix from "node:path/posix";

import { analyze, compileToJs, render } from "./index.js";

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
