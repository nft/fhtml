// Hand-written types for express.js — the "@fhtml/core/express" subpath.

import type { Warning } from "./index.js";

export interface EngineOptions {
  /** The read-only `ctx` root passed to every render (SPEC §9.4). */
  ctx?: unknown;
  /** "min" (default) or "pretty". */
  mode?: "min" | "pretty";
  /** Called after a render that produced compiler warnings. */
  onWarnings?: (warnings: Warning[], path: string) => void;
}

/** An Express view engine — register with `app.engine("fhtml", engine())`
 * and `app.set("view engine", "fhtml")`. Render locals become the
 * template `data` (Express's own `settings`/`cache`/`_locals` keys are
 * filtered out); with Express view caching on, each view's include
 * closure is read from disk once. `init()` runs lazily on first render. */
export function engine(
  opts?: EngineOptions,
): (
  path: string,
  options: Record<string, unknown>,
  callback: (err: unknown, html?: string) => void,
) => Promise<void>;
