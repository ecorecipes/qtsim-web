// Run an options file through the WebAssembly module, as the browser does, and
// print QTSim's output (for parity checks against the native build):
//
//     node tools/run_wasm.mjs [--mode exact|fast1|fast2] options.txt [qtsim.wasm]
//
// Exit status 1 for an options file QTSim rejects (its message is printed), as QTSim.

import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const here = dirname(fileURLToPath(import.meta.url));
let args = process.argv.slice(2);
let mode = 0;
if (args[0] === "--mode") {
  mode = { exact: 0, fast1: 1, fast2: 2, 0: 0, 1: 1, 2: 2 }[args[1]];
  args = args.slice(2);
}
if (process.env.QTSIM_FAST === "1") mode = mode || 1;
if (process.env.QTSIM_FAST === "2") mode = mode || 2;
const file = args[0];
const wasmPath = args[1] ?? join(here, "..", "target", "wasm32-unknown-unknown", "release", "qtsim.wasm");

const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath), {});
const q = instance.exports;
const dec = new TextDecoder();
const str = (ptr, len) => dec.decode(new Uint8Array(q.memory.buffer, ptr, len));

const text = readFileSync(file);
const p = q.qt_alloc(text.length);
new Uint8Array(q.memory.buffer, p, text.length).set(text);
const h = q.qt_new(p, text.length, mode, 12345, 0);
q.qt_dealloc(p, text.length);
const out = [];
if (h === 0) {
  process.stdout.write(str(q.qt_error_ptr(), q.qt_error_len()));
  process.exit(1);
}
out.push(str(q.qt_header_ptr(h), q.qt_header_len(h)));
for (;;) {
  q.qt_step(h, 500);
  out.push(str(q.qt_text_ptr(h), q.qt_text_len(h)));
  q.qt_text_clear(h);
  q.qt_records_clear(h);
  if (q.qt_status(h) !== 0) break;
}
q.qt_free(h);
process.stdout.write(out.join(""));
