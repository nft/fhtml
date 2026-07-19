// Hand-written types for hono.js — the "@fhtml/core/hono" subpath.

import type { Files, InitInput, Warning } from "./index.js";

export interface FhtmlRendererOptions {
  /** Wasm bytes or a compiled module, for runtimes where the default
   * loader can't reach `fhtml.wasm` (e.g. Cloudflare Workers:
   * `import wasm from "@fhtml/core/fhtml.wasm"`). */
  wasm?: InitInput;
  /** The default read-only `ctx` root; a per-render `ctx` argument to
   * `c.render` overrides it. */
  ctx?: unknown;
  /** "min" (default) or "pretty". */
  mode?: "min" | "pretty";
  /** Called after a render that produced compiler warnings. */
  onWarnings?: (warnings: Warning[], entry: string) => void;
}

/** The slice of Hono's Context the middleware touches — structural, so
 * hono is a type-only concern and never a dependency. */
export interface RendererContext {
  setRenderer(
    renderer: (
      name: string,
      data?: unknown,
      ctx?: unknown,
    ) => Response | Promise<Response>,
  ): void;
  html(html: string): Response | Promise<Response>;
}

/** A Hono middleware installing an fhtml renderer: `c.render(name, data,
 * ctx?)` renders `files[name]` (the `.fhtml` extension may be omitted)
 * and responds with `c.html(...)`. */
export function fhtmlRenderer(
  files: Exclude<Files, string>,
  opts?: FhtmlRendererOptions,
): (c: RendererContext, next: () => Promise<void>) => Promise<void>;
