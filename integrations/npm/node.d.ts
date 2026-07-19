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

export interface CompileDirOptions {
  /** `.fhtml` paths (view name = filename stem), or a `{name: path}` map. */
  entries: string[] | Record<string, string>;
  /** Created if missing; never wiped. Treat as generated-owned: files at
   * generated output paths are replaced (untracked files are never
   * pruned, though). */
  outDir: string;
  /** "min" (default) or "pretty". */
  mode?: "min" | "pretty";
  /** Write the `index.js` + `index.d.ts` registry (default true). */
  emitIndex?: boolean;
  /** Write per-view `.d.ts` shims (default true; `index.d.ts` is governed
   * by `emitIndex`). */
  emitDts?: boolean;
  /** Remove outputs of since-removed views (default true). Only files
   * this helper previously emitted — tracked in `.fhtml-manifest.json` —
   * are ever deleted. */
  prune?: boolean;
}

export interface CompileDirResult {
  /** Output files (outDir-relative) whose content changed this run. */
  written: string[];
  /** Outputs already byte-identical — the swap (and the watcher event)
   * was skipped. */
  unchanged: string[];
  /** Previously-emitted files removed because their view is gone. */
  pruned: string[];
  /** Compiler warnings, flattened; `file` is the entry path. */
  warnings: { file: string; msg: string }[];
}

/** Compiles `.fhtml` entries into `outDir` as ES modules plus an
 * `index.js` registry, safely for a live dev loop: no up-front wipe,
 * temp-file + `rename()` for every output, index swapped last, pruning
 * manifest-tracked and run after the fresh index is live. A compile
 * error throws (`FhtmlError` with `file` set) before any write. One
 * writer per `outDir` at a time; per-file atomicity on ordinary local
 * filesystems. Requires `await init()` first. */
export function compileFilesToDir(opts: CompileDirOptions): CompileDirResult;
