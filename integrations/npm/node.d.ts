// Hand-written types for node.js — the "@fhtml/core/node" subpath.

import type {
  Analysis,
  CompileOptions,
  RenderOptions,
  Warning,
} from "./index.js";

/** Reads `path` and every transitively included file into a `{files,
 * entry}` pair ready for `render`/`compileToJs`/`analyze`. Keys are
 * `/`-separated paths relative to the entry's directory. Unreadable
 * included files are left out of the map (the compiler reports the
 * `cannot include` error at the include site); an unreadable entry
 * throws. */
export function loadFiles(path: string): {
  files: Record<string, string>;
  entry: string;
};

/** `render`, but from a file on disk — includes resolved like the CLI.
 * `opts.entry` is ignored (the entry is `path`). */
export function renderFile(
  path: string,
  opts?: Omit<RenderOptions, "entry">,
): { html: string; warnings: Warning[] };

/** `compileToJs`, but from a file on disk. */
export function compileFileToJs(
  path: string,
  opts?: Omit<CompileOptions, "entry">,
): { js: string; warnings: Warning[] };

/** `analyze`, but from a file on disk. */
export function analyzeFile(path: string): Analysis;
