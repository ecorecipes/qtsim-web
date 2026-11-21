// QTSim in a Web Worker: runs the WebAssembly module off the page's thread and
// sends what it produces to the page in batches.
//
// Messages in:  {type:"init", wasm}        the module's bytes
//               {type:"new", options, mode, seedLo, seedHi, keepText}
//               {type:"run"} {type:"pause"} {type:"step", n} {type:"free"}
// Messages out: {type:"ready", recordLen}
//               {type:"created", header, file, params, popSize, status}
//               {type:"batch", records, text, pheno, fit, gen, status, theta, omega, rate}
//               {type:"ended", status, gen} {type:"error", message}

"use strict";

let q = null; // the module's exports
let h = 0; // the current run
let running = false;
let recordLen = 12;
const dec = new TextDecoder();
const enc = new TextEncoder();

const str = (ptr, len) => (len ? dec.decode(new Uint8Array(q.memory.buffer, ptr, len)) : "");

function free() {
  running = false;
  if (h) q.qt_free(h);
  h = 0;
}

function params() {
  return {
    theta: q.qt_param(h, 0),
    omega: q.qt_param(h, 1),
    envVar: q.qt_param(h, 2),
    muteVar: q.qt_param(h, 3),
    phenotypeVar: q.qt_param(h, 4),
    heritability: q.qt_param(h, 5),
    analyticOmega: q.qt_param(h, 6),
  };
}

// Send what has accumulated since the last batch (records, text, the population)
function flush(gens, ms) {
  if (!h) return;
  const n = q.qt_records_len(h);
  const records = new Float64Array(n);
  if (n) records.set(new Float64Array(q.memory.buffer, q.qt_records_ptr(h), n));
  q.qt_records_clear(h);
  const text = str(q.qt_text_ptr(h), q.qt_text_len(h));
  q.qt_text_clear(h);
  const N = q.qt_pop_len(h);
  const pheno = new Float64Array(N);
  const fit = new Float64Array(N);
  if (N) {
    pheno.set(new Float64Array(q.memory.buffer, q.qt_pop_ptr(h, 0), N));
    fit.set(new Float64Array(q.memory.buffer, q.qt_pop_ptr(h, 2), N));
  }
  postMessage(
    {
      type: "batch",
      records,
      text,
      pheno,
      fit,
      gen: q.qt_generation(h),
      status: q.qt_status(h),
      theta: q.qt_param(h, 0),
      omega: q.qt_param(h, 1),
      rate: gens && ms ? (1000 * gens) / ms : 0,
    },
    [records.buffer, pheno.buffer, fit.buffer]
  );
}

function loop() {
  if (!running || !h) return;
  const t0 = performance.now();
  let gens = 0;
  // about 40 ms of work, a generation at a time, then let messages in
  while (performance.now() - t0 < 40) {
    const k = q.qt_step(h, 1);
    gens += k;
    if (k === 0 || q.qt_status(h) !== 0) break;
  }
  flush(gens, performance.now() - t0);
  if (q.qt_status(h) !== 0) {
    running = false;
    postMessage({ type: "ended", status: q.qt_status(h), gen: q.qt_generation(h) });
    return;
  }
  setTimeout(loop, 0);
}

onmessage = async (e) => {
  const m = e.data;
  try {
    switch (m.type) {
      case "init": {
        const { instance } = await WebAssembly.instantiate(m.wasm, {});
        q = instance.exports;
        recordLen = q.qt_record_len();
        postMessage({ type: "ready", recordLen });
        break;
      }
      case "new": {
        free();
        const b = enc.encode(m.options);
        const p = q.qt_alloc(b.length);
        new Uint8Array(q.memory.buffer, p, b.length).set(b);
        h = q.qt_new(p, b.length, m.mode, m.seedLo >>> 0, m.seedHi >>> 0);
        q.qt_dealloc(p, b.length);
        if (!h) {
          postMessage({ type: "error", message: str(q.qt_error_ptr(), q.qt_error_len()) });
          return;
        }
        q.qt_keep_text(h, m.keepText ? 1 : 0);
        postMessage({
          type: "created",
          header: str(q.qt_header_ptr(h), q.qt_header_len(h)),
          file: str(q.qt_file_ptr(h), q.qt_file_len(h)),
          params: params(),
          popSize: q.qt_pop_len(h),
          status: q.qt_status(h),
        });
        break;
      }
      case "run":
        if (h && q.qt_status(h) === 0 && !running) {
          running = true;
          loop();
        }
        break;
      case "pause":
        running = false;
        break;
      case "step":
        if (h && q.qt_status(h) === 0) {
          running = false;
          const t0 = performance.now();
          const k = q.qt_step(h, m.n || 1);
          flush(k, performance.now() - t0);
          if (q.qt_status(h) !== 0) postMessage({ type: "ended", status: q.qt_status(h), gen: q.qt_generation(h) });
        }
        break;
      case "free":
        free();
        break;
    }
  } catch (err) {
    running = false;
    postMessage({ type: "error", message: String(err && err.message ? err.message : err) });
  }
};
