// Raw ABI smoke test: drives the
// .wasm with the bare WebAssembly API — no glue — to prove the three-export
// memory contract itself. Build first:
//
//   cd ../crate && cargo build --release --target wasm32-unknown-unknown
//
// Run: node raw-abi.mjs  (from this directory)

import { readFile } from "node:fs/promises";
import assert from "node:assert/strict";

const wasmPath = new URL(
  "../crate/target/wasm32-unknown-unknown/release/fhtml_wasm.wasm",
  import.meta.url,
);
const { instance } = await WebAssembly.instantiate(await readFile(wasmPath));
const { fh_alloc, fh_dealloc, fh_call, memory } = instance.exports;

// One reusable 4-byte slot for the out-param, like a real host would keep.
const outLenPtr = fh_alloc(4);

function call(request) {
  const req = new TextEncoder().encode(JSON.stringify(request));
  const reqPtr = fh_alloc(req.length);
  new Uint8Array(memory.buffer, reqPtr, req.length).set(req);
  const resPtr = fh_call(reqPtr, req.length, outLenPtr);
  // Re-read memory.buffer after the call — growth invalidates old views.
  const resLen = new DataView(memory.buffer).getUint32(outLenPtr, true);
  let text;
  try {
    // decode() copies, so freeing before JSON.parse is sound.
    text = new TextDecoder().decode(new Uint8Array(memory.buffer, resPtr, resLen));
  } finally {
    fh_dealloc(resPtr, resLen);
    fh_dealloc(reqPtr, req.length);
  }
  return JSON.parse(text);
}

// ---- version --------------------------------------------------------------

const version = call({ fn: "version" });
assert.match(version.ok.version, /^\d+\.\d+\.\d+/);

// ---- templated multi-file render (the gate) --------------------------------

const files = {
  "lib.fhtml": 'def badge(label)\n  span rounded "{label}"\n',
  "main.fhtml": 'include ./lib\n\ndiv grid\n  +badge(label={name})\n  p "{ctx.who}"\n',
};
const rendered = call({
  fn: "render",
  files,
  entry: "main.fhtml",
  data: { name: "hi" },
  ctx: { who: "us" },
});
// Byte-for-byte what the native CLI prints for the same input (--min).
assert.equal(
  rendered.ok.html,
  '<div class="grid"><span class="rounded">hi</span><p>us</p></div>',
);
assert.deepEqual(rendered.ok.warnings, []);

// ---- compileToJs / format / analyze ---------------------------------------

const js = call({ fn: "compileToJs", files, entry: "main.fhtml" });
assert.match(js.ok.js, /export default/);

const formatted = call({ fn: "format", src: "div\n    span   \"hi\"\n" });
// Canonical form: 2-space indent, `.` for div.
assert.equal(formatted.ok.src, '.\n  span "hi"\n');

const analysis = call({ fn: "analyze", files, entry: "main.fhtml" });
assert.equal(analysis.ok.error, null);
const badge = analysis.ok.defs.find((d) => d.name === "badge");
assert.equal(badge.file, "lib.fhtml");
assert.deepEqual(badge.nameSpan, { line: 1, col: 5, len: 5 });
assert.equal(analysis.ok.includes[0].resolved, "lib.fhtml");

// ---- errors are data, never traps -----------------------------------------

const broken = call({ fn: "render", files: { "m.fhtml": 'span "unclosed\n' } });
assert.equal(broken.err.line, 1);
assert.match(broken.err.msg, /unclosed string/);

const missing = call({ fn: "render", files, entry: "nope.fhtml" });
assert.match(missing.err.msg, /not in the file map/);

const unknown = call({ fn: "nope" });
assert.match(unknown.err.msg, /unknown fn `nope`/);

// Malformed JSON straight onto the ABI (not via call() — send raw bytes).
{
  const req = new TextEncoder().encode("not json at all");
  const reqPtr = fh_alloc(req.length);
  new Uint8Array(memory.buffer, reqPtr, req.length).set(req);
  const resPtr = fh_call(reqPtr, req.length, outLenPtr);
  const resLen = new DataView(memory.buffer).getUint32(outLenPtr, true);
  const text = new TextDecoder().decode(new Uint8Array(memory.buffer, resPtr, resLen));
  fh_dealloc(resPtr, resLen);
  fh_dealloc(reqPtr, req.length);
  assert.match(JSON.parse(text).err.msg, /not valid JSON/);
}

// ---- memory hygiene: many calls, stable growth ----------------------------

const before = memory.buffer.byteLength;
for (let i = 0; i < 2000; i++) {
  call({ fn: "render", files, entry: "main.fhtml", data: { name: `n${i}` } });
}
const grown = memory.buffer.byteLength - before;
// Freed buffers must be reused — 2000 render round-trips should not grow
// linear memory by more than a couple of pages' worth of slack.
assert.ok(grown <= 1 << 20, `leaked: memory grew ${grown} bytes over 2000 calls`);

console.log(`raw-abi.mjs: all assertions passed (memory growth over 2000 calls: ${grown} bytes)`);
