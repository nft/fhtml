// Hand-written types for index.js.

/** A source string (single file), or a `{name: source}` map — includes
 * resolve against the map, `entry` names the file to process. */
export type Files = string | Record<string, string>;

export type InitInput = BufferSource | WebAssembly.Module;

/** Instantiates the wasm; idempotent. With no argument, loads
 * `fhtml.wasm` from next to the module (`node:fs` for `file:` URLs,
 * `fetch` otherwise). Pass bytes or a compiled module where neither
 * works (e.g. Cloudflare Workers' native wasm imports). */
export function init(input?: InitInput): Promise<void>;

/** A compiler warning. Today just the formatted message
 * (`line:col: warning: …`); the object shape leaves room for structure. */
export interface Warning {
  msg: string;
}

export interface RenderOptions {
  /** Which file of the map to render. Required for multi-file maps. */
  entry?: string;
  /** The template data root (SPEC §9–§10). Missing names render null. */
  data?: unknown;
  /** The read-only `ctx` root (SPEC §9.4). */
  ctx?: unknown;
  /** "min" (default) or "pretty". */
  mode?: "min" | "pretty";
}

export function render(
  files: Files,
  opts?: RenderOptions,
): { html: string; warnings: Warning[] };

export interface CompileOptions {
  entry?: string;
  mode?: "min" | "pretty";
}

/** Compiles to a self-contained ES module exporting
 * `(data, ctx = {}) => string`. */
export function compileToJs(
  files: Files,
  opts?: CompileOptions,
): { js: string; warnings: Warning[] };

export interface FormatOptions {
  /** "preserve" (default) keeps the authored form; "contract" rewrites
   * classes into `#!shorthand` codes where they round-trip; "expand"
   * rewrites codes back to full classes (SPEC §3.2). */
  shorthand?: "preserve" | "expand" | "contract";
}

/** Reformats to the canonical style; returns the formatted source. */
export function format(src: string, opts?: FormatOptions): string;

export interface Span {
  /** 1-based. */
  line: number;
  /** 1-based. */
  col: number;
  len: number;
}

export interface Diag extends Span {
  msg: string;
}

export interface ParamSym {
  name: string;
  nameSpan: Span;
  /** The default-value expression source, if the param has one. */
  default?: string;
}

export interface DefSym {
  name: string;
  nameSpan: Span;
  endLine: number;
  params: ParamSym[];
  /** The map key the def came from; absent for the entry file itself. */
  file?: string;
}

export interface ArgSym {
  name: string;
  span: Span;
}

export interface CallSym {
  name: string;
  nameSpan: Span;
  args: ArgSym[];
}

export interface IncludeSym {
  path: string;
  span: Span;
  /** The map key the include resolved to; absent when unresolved. */
  resolved?: string;
}

export interface Analysis {
  defs: DefSym[];
  calls: CallSym[];
  includes: IncludeSym[];
  warnings: Diag[];
  /** The parse error, or null — analysis itself never throws. */
  error: Diag | null;
}

export interface AnalyzeOptions {
  entry?: string;
}

/** Full document analysis — the same data the native LSP serves. */
export function analyze(files: Files, opts?: AnalyzeOptions): Analysis;

/** The compiler version compiled into the wasm. */
export function version(): string;

/** A compile/render error, thrown by `render`/`compileToJs`/`format`.
 * `line`/`col` are 1-based and null for non-source errors (bad request,
 * missing entry). */
export class FhtmlError extends Error {
  line: number | null;
  col: number | null;
  /** The entry path the error came from, when a batch helper
   * (`compileFilesToDir`) can attribute it; absent otherwise. */
  file?: string;
}
