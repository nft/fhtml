// fhtml as WebAssembly: hand-written
// ESM glue over the three-export ABI in crate/src/lib.rs. No bundler, no
// wasm-bindgen, no dependencies — `fhtml.wasm` next to this file is the
// whole runtime.
//
// ESM-only, and deliberately no top-level await: `init()` is an explicit
// call, so Node >= 20.19 can `require()` this module for CJS consumers
// without a dual build (two builds would mean two wasm instances).

/** A compile/render error from the wasm side, as data. `line`/`col` are
 * 1-based and null for non-source errors (bad request, missing entry). */
export class FhtmlError extends Error {
  constructor(err) {
    super(err.msg);
    this.name = "FhtmlError";
    this.line = err.line ?? null;
    this.col = err.col ?? null;
  }
}

let exports = null; // { fh_alloc, fh_dealloc, fh_call, memory } once ready
let outLenPtr = 0; // one reusable 4-byte slot for fh_call's out-param
let pending = null; // in-flight init, shared by concurrent callers

/**
 * Instantiates the wasm. Idempotent — after the first success, later calls
 * (with or without an argument) resolve immediately. With no argument the
 * module loads `fhtml.wasm` from next to this file (`node:fs` for file:
 * URLs, `fetch` otherwise); pass bytes or a compiled `WebAssembly.Module`
 * where neither works, e.g. Cloudflare Workers' native wasm imports.
 */
export async function init(input) {
  if (exports) return;
  pending ??= instantiate(input).then((instance) => {
    exports = instance.exports;
    outLenPtr = exports.fh_alloc(4);
  });
  try {
    await pending;
  } catch (e) {
    pending = null; // a failed load (e.g. fetch) may be retried
    throw e;
  }
}

async function instantiate(input) {
  if (input instanceof WebAssembly.Module) {
    return new WebAssembly.Instance(input);
  }
  if (input === undefined) {
    const url = new URL("fhtml.wasm", import.meta.url);
    if (url.protocol === "file:") {
      const { readFile } = await import("node:fs/promises");
      input = await readFile(url);
    } else if (typeof WebAssembly.instantiateStreaming === "function") {
      const { instance } = await WebAssembly.instantiateStreaming(fetch(url));
      return instance;
    } else {
      input = await (await fetch(url)).arrayBuffer();
    }
  }
  const { instance } = await WebAssembly.instantiate(input);
  return instance;
}

/** Renders to HTML. `files` is a source string, or a `{name: source}` map
 * with `opts.entry` naming the file to render (includes resolve against
 * the map). Returns `{html, warnings}`; throws `FhtmlError` on a compile
 * error. */
export function render(files, opts = {}) {
  return call({ fn: "render", ...fileArgs(files, opts), data: opts.data, ctx: opts.ctx, mode: opts.mode });
}

/** Compiles to a self-contained ES module exporting
 * `(data, ctx = {}) => string` — the render path itself carries no wasm.
 * Returns `{js, warnings}`. */
export function compileToJs(files, opts = {}) {
  return call({ fn: "compileToJs", ...fileArgs(files, opts), mode: opts.mode });
}

/** Reformats source to the canonical style; `opts.shorthand` is
 * "preserve" (default), "expand", or "contract". Returns the formatted
 * source. */
export function format(src, opts = {}) {
  return call({ fn: "format", src, shorthand: opts.shorthand }).src;
}

/** Full document analysis — defs (cross-file), calls, includes,
 * diagnostics — the same data the native LSP serves. Never throws on
 * broken source: the parse error comes back as `analysis.error`. */
export function analyze(files, opts = {}) {
  return call({ fn: "analyze", ...fileArgs(files, opts) });
}

/** The compiler version compiled into the wasm. */
export function version() {
  return call({ fn: "version" }).version;
}

// ---- the boundary ---------------------------------------------------------

function call(request) {
  if (!exports) {
    throw new Error("fhtml: not initialized — `await init()` first");
  }
  const { fh_alloc, fh_dealloc, fh_call, memory } = exports;
  const req = new TextEncoder().encode(JSON.stringify(request));
  const reqPtr = fh_alloc(req.length);
  new Uint8Array(memory.buffer, reqPtr, req.length).set(req);
  let resPtr = 0;
  let resLen = 0;
  let text;
  try {
    resPtr = fh_call(reqPtr, req.length, outLenPtr);
    // Re-read memory.buffer after the call — growth invalidates old views.
    resLen = new DataView(memory.buffer).getUint32(outLenPtr, true);
    text = new TextDecoder().decode(new Uint8Array(memory.buffer, resPtr, resLen));
  } finally {
    // decode() copies, so parsing after the free is sound — and stays
    // sound only because of that copy.
    if (resPtr) fh_dealloc(resPtr, resLen);
    fh_dealloc(reqPtr, req.length);
  }
  const resp = JSON.parse(text);
  if (resp.err) throw new FhtmlError(resp.err);
  return resp.ok;
}

/** Normalizes the `files`/`entry` pair. Keys cross the boundary with `/`
 * separators: `std::path` on wasm32 is Unix-flavored, so a Windows host's
 * backslashed keys would otherwise be opaque single-component names. */
function fileArgs(files, opts) {
  if (typeof files === "string") {
    return { files: { "main.fhtml": files } };
  }
  const map = {};
  for (const [name, src] of Object.entries(files)) {
    map[name.replaceAll("\\", "/")] = src;
  }
  return { files: map, entry: opts.entry?.replaceAll("\\", "/") };
}
