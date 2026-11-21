// Bundle the app into one self-contained HTML file (opens from disk, no server):
// the stylesheet, the page script, the worker's source and the WebAssembly module
// (base64) are inlined.
//
//     node tools/bundle.mjs [out.html]      (default dist/qtsim.html)

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const web = join(root, "web");
const out = process.argv[2] ?? join(root, "dist", "qtsim.html");
const wasmPath = join(root, "target", "wasm32-unknown-unknown", "release", "qtsim.wasm");

const read = (p) => readFileSync(p, "utf8");
const safe = (js) => js.replace(/<\/script/gi, "<\\/script");

let html = read(join(web, "index.html"));
const css = read(join(web, "style.css"));
const app = read(join(web, "app.js"));
const worker = read(join(web, "worker.js"));
const wasm = readFileSync(wasmPath).toString("base64");

const link = '<link rel="stylesheet" href="style.css">';
const script = '<script src="app.js"></script>';
if (!html.includes(link) || !html.includes(script)) throw new Error("index.html: stylesheet link or script tag not found");

html = html.replace(link, () => `<style>\n${css}</style>`);
html = html.replace(
  script,
  () =>
    `<script>window.QTSIM_WASM_B64 = "${wasm}";\nwindow.QTSIM_WORKER_SRC = ${safe(JSON.stringify(worker))};</script>\n` +
    `<script>\n${safe(app)}</script>`
);

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, html);
console.log(`wrote ${out} (${(html.length / 1024).toFixed(0)} KB)`);
