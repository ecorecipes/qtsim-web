// QTSim Explorer: the page. The simulation runs in worker.js (QTSim compiled to
// WebAssembly); this file holds the settings, the charts and the exports.

"use strict";

// ------------------------------------------------------------------ presets

const BASE = {
  PopSize: 1000, Ploidy: 2, Generations: 12000, Sample: 1, Seed: 1001,
  NoQTL: 100, MutationRatePerGamete: 0.0001, MutationScalar: 0.001, RecombinationPoissonMean: 3,
  PhenotypeVar: 0.02, Heritability: 0.6, omegaMode: "fixed", InitFitnessSD: 1.0, StandardisePhenotype: false,
  OptimumMoveType: "continuous", MoveGen: 10000, MoveSD: 0.002, changeOn: false, ChangeGen: 0, ChangeSD: 0.1,
};

const PRESETS = [
  { name: "Published design: 100 loci, slow move", set: {} },
  { name: "Published design: 1000 loci", set: { NoQTL: 1000, RecombinationPoissonMean: 5 } },
  { name: "Published design: 5000 loci", set: { NoQTL: 5000 } },
  { name: "Fast move: extinction", set: { NoQTL: 500, RecombinationPoissonMean: 5, Generations: 3000, MoveGen: 2000, MoveSD: 0.05 } },
  { name: "Near the threshold", set: { Generations: 3000, MoveGen: 2000, MoveSD: 0.009 } },
  { name: "Stabilising selection only", set: { Generations: 4000, OptimumMoveType: "none" } },
  {
    name: "Single-locus protocol (QTSim's demo)",
    set: {
      NoQTL: 1, Generations: 6500, Seed: 101, PhenotypeVar: 0.01, Heritability: 0.5, MutationScalar: 0.001,
      MutationRatePerGamete: 0.0002, RecombinationPoissonMean: 1, omegaMode: "analytic",
      OptimumMoveType: "phenotype", MoveGen: 5000, MoveSD: 2.0, changeOn: true, ChangeGen: 5000, ChangeSD: 0.17,
    },
  },
  {
    name: "Haploid, many mutations",
    set: {
      PopSize: 200, Ploidy: 1, NoQTL: 257, Generations: 600, Seed: 4711, MutationRatePerGamete: 0.05,
      RecombinationPoissonMean: 12, Heritability: 0.5, omegaMode: "analytic", OptimumMoveType: "fitness", MoveGen: 300, MoveSD: 0.3,
    },
  },
];

const MODE_HINT = [
  "Exact: QTSim's own output, byte for byte, for the same settings and seed.",
  "Fast 1: genotypes from cached sums of blocks of loci. QTSim's random draws; output almost always identical, much faster with many loci.",
  "Fast 2: also other random numbers (independent streams). Same statistics in distribution as QTSim, not the same runs; the fastest.",
];

const COLS = ["generation", "phenotype_mean", "phenotype_ratio", "genotype_var", "survivors", "optimum", "width",
  "surviving_slope", "standardised_slope", "w", "genotype_mean", "phenotype_var"];

// ------------------------------------------------------------------ helpers

const $ = (id) => document.getElementById(id);
const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim();

function fmt(x) {
  if (x === undefined || x === null || Number.isNaN(x)) return "–";
  if (!Number.isFinite(x)) return x > 0 ? "∞" : "−∞";
  const a = Math.abs(x);
  if (a !== 0 && (a >= 1e5 || a < 1e-3)) return x.toExponential(2).replace("-", "−");
  return x.toPrecision(4).replace(/^-/, "−");
}

function download(name, text, type = "text/plain") {
  const url = URL.createObjectURL(new Blob([text], { type }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// ------------------------------------------------------------------ settings ↔ options file

const FIELDS = ["PopSize", "Ploidy", "Generations", "Sample", "Seed", "NoQTL", "MutationRatePerGamete", "MutationScalar",
  "RecombinationPoissonMean", "PhenotypeVar", "Heritability", "omegaMode", "InitFitnessSD", "StandardisePhenotype",
  "OptimumMoveType", "MoveGen", "MoveSD", "changeOn", "ChangeGen", "ChangeSD"];

function setForm(v) {
  for (const k of FIELDS) {
    const el = $(k);
    if (!el || v[k] === undefined) continue;
    if (el.type === "checkbox") el.checked = !!v[k];
    else el.value = String(v[k]);
  }
  syncForm();
}

function getForm() {
  const v = {};
  for (const k of FIELDS) {
    const el = $(k);
    v[k] = el.type === "checkbox" ? el.checked : el.value;
  }
  return v;
}

const num = (s) => String(Number(s));

function optionsText(v) {
  const L = [];
  L.push("# QTSim options, written by QTSim Explorer");
  L.push(`Sample ${num(v.Sample)}`);
  L.push(`PopSize ${num(v.PopSize)}`);
  L.push(`NoQTL ${num(v.NoQTL)}`);
  L.push(`Ploidy ${num(v.Ploidy)}`);
  L.push(`Generations ${num(v.Generations)}`);
  L.push(`PhenotypeVar ${num(v.PhenotypeVar)}`);
  L.push(`Heritability ${num(v.Heritability)}`);
  L.push(`MutationScalar ${num(v.MutationScalar)}`);
  L.push(`MutationRatePerGamete ${num(v.MutationRatePerGamete)}`);
  L.push(`RecombinationPoissonMean ${num(v.RecombinationPoissonMean)}`);
  if (v.omegaMode === "fixed") L.push(`InitFitnessSD ${num(v.InitFitnessSD)}`);
  if (v.OptimumMoveType !== "none") {
    L.push(`MoveFitnessSD ${num(v.MoveGen)} ${num(v.MoveSD)}`);
    L.push(`OptimumMoveType ${v.OptimumMoveType}`);
  }
  if (v.changeOn) L.push(`ChangeFitnessSD ${num(v.ChangeGen)} ${num(v.ChangeSD)}`);
  if (v.StandardisePhenotype) L.push("StandardisePhenotype");
  L.push(`Seed ${num(v.Seed)}`);
  return L.join("\n") + "\n";
}

// Best-effort reading of an options file into the settings (the file itself is what runs)
function formFromText(text) {
  const v = { ...BASE, omegaMode: "analytic", OptimumMoveType: "none", changeOn: false, StandardisePhenotype: false };
  let moveType = "phenotype"; // QTSim's default
  let moves = false;
  for (const raw of text.split(/\r?\n/)) {
    const t = raw.trim().split(/\s+/);
    if (!t[0] || t[0].startsWith("#")) continue;
    const k = t[0].toLowerCase();
    const simple = { sample: "Sample", popsize: "PopSize", noqtl: "NoQTL", ploidy: "Ploidy", generations: "Generations",
      phenotypevar: "PhenotypeVar", heritability: "Heritability", mutationscalar: "MutationScalar",
      mutationratepergamete: "MutationRatePerGamete", recombinationpoissonmean: "RecombinationPoissonMean", seed: "Seed" };
    if (simple[k] && t[1] !== undefined) v[simple[k]] = t[1];
    else if (k === "initfitnesssd") { v.omegaMode = "fixed"; v.InitFitnessSD = t[1]; }
    else if (k === "movefitnesssd") { moves = true; v.MoveGen = t[1]; v.MoveSD = t[2]; }
    else if (k === "optimummovetype" && t[1]) moveType = t[1].toLowerCase();
    else if (k === "changefitnesssd") { v.changeOn = true; v.ChangeGen = t[1]; v.ChangeSD = t[2]; }
    else if (k === "standardisephenotype") v.StandardisePhenotype = !v.StandardisePhenotype;
  }
  if (moves) v.OptimumMoveType = ["continuous", "phenotype", "fitness"].includes(moveType) ? moveType : "phenotype";
  return v;
}

function analyticOmega(v) {
  const vp = Number(v.PhenotypeVar), h2 = Number(v.Heritability), m = Number(v.MutationScalar);
  return (vp * h2) / Math.sqrt(m * vp * (1 - h2));
}

let textEdited = false;

function syncForm() {
  const v = getForm();
  $("omegaRow").hidden = v.omegaMode !== "fixed";
  $("omegaHint").textContent = v.omegaMode === "fixed"
    ? ""
    : `ω = h²V_P / √(m(1 − h²)V_P) = ${fmt(analyticOmega(v))}`;
  const moves = v.OptimumMoveType !== "none";
  for (const el of document.querySelectorAll(".move")) el.hidden = !moves;
  $("moveLabel").textContent = { continuous: "Per generation", phenotype: "Lead (SDs of V_P)", fitness: "Lead (widths ω)" }[v.OptimumMoveType] || "";
  for (const el of document.querySelectorAll(".change")) el.hidden = !v.changeOn;
  $("modeHint").textContent = MODE_HINT[mode()];
  if (!textEdited) $("optText").value = optionsText(v);
  markStale();
}

const mode = () => Number(document.querySelector('input[name="mode"]:checked').value);

// ------------------------------------------------------------------ the records

class Store {
  constructor(width) {
    this.w = width;
    this.n = 0;
    this.cap = 1024;
    this.cols = Array.from({ length: width }, () => new Float64Array(this.cap));
  }
  push(flat) {
    const k = flat.length / this.w;
    if (this.n + k > this.cap) {
      while (this.n + k > this.cap) this.cap *= 2;
      this.cols = this.cols.map((c) => { const d = new Float64Array(this.cap); d.set(c.subarray(0, this.n)); return d; });
    }
    for (let r = 0; r < k; r++) for (let j = 0; j < this.w; j++) this.cols[j][this.n + r] = flat[r * this.w + j];
    this.n += k;
  }
  col(j) { return this.cols[j].subarray(0, this.n); }
  last(j) { return this.n ? this.cols[j][this.n - 1] : NaN; }
}

// ------------------------------------------------------------------ charts

function niceTicks(lo, hi, n) {
  if (!(hi > lo)) { const d = Math.abs(lo) || 1; lo -= d / 2; hi += d / 2; }
  const raw = (hi - lo) / n;
  const p = Math.pow(10, Math.floor(Math.log10(raw)));
  const step = [1, 2, 2.5, 5, 10].map((m) => m * p).find((s) => s >= raw) || 10 * p;
  const t = [];
  for (let v = Math.ceil(lo / step) * step; v <= hi + step * 1e-9; v += step) t.push(Math.abs(v) < step * 1e-9 ? 0 : v);
  return t;
}

function tickLabel(v) {
  const a = Math.abs(v);
  if (a !== 0 && (a >= 1e5 || a < 1e-3)) return v.toExponential(1).replace("-", "−");
  return String(Number(v.toPrecision(6))).replace(/^-/, "−");
}

function setupCanvas(canvas) {
  const dpr = window.devicePixelRatio || 1;
  const r = canvas.getBoundingClientRect();
  const w = Math.max(10, Math.round(r.width * dpr)), h = Math.max(10, Math.round(r.height * dpr));
  if (canvas.width !== w || canvas.height !== h) { canvas.width = w; canvas.height = h; }
  const ctx = canvas.getContext("2d");
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return { ctx, W: r.width, H: r.height };
}

function axes(ctx, W, H, x0, x1, y0, y1, pad) {
  const ink = css("--muted"), line = css("--line");
  ctx.font = "11px " + css("--font");
  ctx.fillStyle = ink;
  ctx.strokeStyle = line;
  ctx.lineWidth = 1;
  const X = (x) => pad.l + ((x - x0) / (x1 - x0 || 1)) * (W - pad.l - pad.r);
  const Y = (y) => H - pad.b - ((y - y0) / (y1 - y0 || 1)) * (H - pad.t - pad.b);
  ctx.textAlign = "right";
  ctx.textBaseline = "middle";
  for (const t of niceTicks(y0, y1, 4)) {
    if (t < y0 - 1e-12 || t > y1 + 1e-12) continue;
    const y = Math.round(Y(t)) + 0.5;
    ctx.beginPath(); ctx.moveTo(pad.l, y); ctx.lineTo(W - pad.r, y); ctx.stroke();
    ctx.fillText(tickLabel(t), pad.l - 6, y);
  }
  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  for (const t of niceTicks(x0, x1, 5)) {
    if (t < x0 || t > x1) continue;
    ctx.fillText(tickLabel(t), X(t), H - pad.b + 5);
  }
  return { X, Y };
}

class LineChart {
  constructor(canvas, series, opts = {}) {
    this.canvas = canvas;
    this.series = series; // [{get(store) -> Float64Array, color: css var, label}]
    this.opts = opts;
    this.hover = null;
    this.readout = document.createElement("span");
    this.readout.className = "readout";
    canvas.parentElement.querySelector("figcaption").appendChild(this.readout);
    canvas.addEventListener("mousemove", (e) => { this.hover = e.offsetX; schedule(); });
    canvas.addEventListener("mouseleave", () => { this.hover = null; this.readout.textContent = ""; schedule(); });
  }

  draw(store, xmax) {
    const { ctx, W, H } = setupCanvas(this.canvas);
    ctx.clearRect(0, 0, W, H);
    const pad = { l: 52, r: 10, t: 6, b: 20 };
    const x = store.col(0);
    const ys = this.series.map((s) => s.get(store));
    const n = x.length;
    const x0 = 0, x1 = Math.max(xmax || 1, n ? x[n - 1] : 1);
    let y0 = Infinity, y1 = -Infinity;
    for (const y of ys) for (let i = 0; i < n; i++) { const v = y[i]; if (Number.isFinite(v)) { if (v < y0) y0 = v; if (v > y1) y1 = v; } }
    if (this.opts.zero) { y0 = Math.min(0, y0); y1 = Math.max(0, y1); }
    if (!Number.isFinite(y0)) { y0 = 0; y1 = 1; }
    if (y1 === y0) { const d = Math.abs(y0) * 0.1 || 0.1; y0 -= d; y1 += d; }
    else { const m = (y1 - y0) * 0.06; y1 += m; if (!(this.opts.zero && y0 === 0)) y0 -= m; }
    const { X, Y } = axes(ctx, W, H, x0, x1, y0, y1, pad);

    const cols = Math.max(1, Math.floor(W - pad.l - pad.r));
    this.series.forEach((s, si) => {
      const y = ys[si];
      ctx.strokeStyle = css(s.color);
      ctx.lineWidth = s.width || 1.5;
      ctx.setLineDash(s.dash || []);
      ctx.beginPath();
      if (n <= cols * 2) {
        let started = false;
        for (let i = 0; i < n; i++) {
          if (!Number.isFinite(y[i])) { started = false; continue; }
          const px = X(x[i]), py = Y(y[i]);
          if (!started) { ctx.moveTo(px, py); started = true; } else ctx.lineTo(px, py);
        }
      } else {
        // one vertical stroke per pixel column, from the column's least to greatest value
        let i = 0;
        for (let c = 0; c < cols && i < n; c++) {
          const xe = x0 + ((c + 1) / cols) * (x1 - x0);
          let lo = Infinity, hi = -Infinity, first = NaN, lastv = NaN;
          while (i < n && x[i] <= xe) {
            const v = y[i++];
            if (!Number.isFinite(v)) continue;
            if (Number.isNaN(first)) first = v;
            lastv = v;
            if (v < lo) lo = v;
            if (v > hi) hi = v;
          }
          if (!Number.isFinite(lo)) continue;
          const px = pad.l + c + 0.5;
          if (c === 0) ctx.moveTo(px, Y(first)); else ctx.lineTo(px, Y(first));
          ctx.lineTo(px, Y(lo)); ctx.lineTo(px, Y(hi)); ctx.lineTo(px, Y(lastv));
        }
      }
      ctx.stroke();
      ctx.setLineDash([]);
    });

    // hover readout
    if (this.hover !== null && n) {
      const gx = x0 + ((this.hover - pad.l) / (W - pad.l - pad.r)) * (x1 - x0);
      let a = 0, b = n - 1;
      while (b - a > 1) { const m = (a + b) >> 1; if (x[m] < gx) a = m; else b = m; }
      const i = Math.abs(x[a] - gx) <= Math.abs(x[b] - gx) ? a : b;
      const px = Math.round(X(x[i])) + 0.5;
      ctx.strokeStyle = css("--muted");
      ctx.lineWidth = 1;
      ctx.beginPath(); ctx.moveTo(px, pad.t); ctx.lineTo(px, H - pad.b); ctx.stroke();
      this.readout.textContent = `gen ${x[i]} · ` + this.series.map((s, si) => `${s.label} ${fmt(ys[si][i])}`).join(" · ");
    }

    if (this.opts.legend) {
      ctx.font = "11px " + css("--font");
      ctx.textAlign = "left";
      ctx.textBaseline = "top";
      let lx = pad.l + 8;
      for (const s of this.series) {
        ctx.fillStyle = css(s.color);
        ctx.fillRect(lx, pad.t + 6, 10, 2);
        ctx.fillStyle = css("--ink");
        ctx.fillText(s.label, lx + 14, pad.t + 1);
        lx += ctx.measureText(s.label).width + 30;
      }
    }
  }
}

class PopChart {
  constructor(canvas) { this.canvas = canvas; }

  draw(pheno, theta, omega) {
    const { ctx, W, H } = setupCanvas(this.canvas);
    ctx.clearRect(0, 0, W, H);
    const pad = { l: 52, r: 10, t: 6, b: 20 };
    if (!pheno || !pheno.length) return;
    let lo = Infinity, hi = -Infinity, sum = 0;
    for (const v of pheno) { if (v < lo) lo = v; if (v > hi) hi = v; sum += v; }
    const mean = sum / pheno.length;
    const span = Math.max(hi - lo, 1e-12);
    lo -= span * 0.1; hi += span * 0.1;
    if (Number.isFinite(theta)) { lo = Math.min(lo, theta - span * 0.1); hi = Math.max(hi, theta + span * 0.1); }
    const bins = 48;
    const counts = new Array(bins).fill(0);
    for (const v of pheno) counts[Math.min(bins - 1, Math.max(0, Math.floor(((v - lo) / (hi - lo)) * bins)))]++;
    const cmax = Math.max(...counts, 1);
    const { X, Y } = axes(ctx, W, H, lo, hi, 0, cmax * 1.08, pad);

    ctx.fillStyle = css("--series-1");
    ctx.globalAlpha = 0.55;
    for (let b = 0; b < bins; b++) {
      if (!counts[b]) continue;
      const xa = X(lo + (b / bins) * (hi - lo)), xb = X(lo + ((b + 1) / bins) * (hi - lo));
      ctx.fillRect(xa + 0.5, Y(counts[b]), Math.max(1, xb - xa - 1), Y(0) - Y(counts[b]));
    }
    ctx.globalAlpha = 1;

    // the fitness function exp(−(z − θ)²/(2ω²)), scaled to the tallest bar
    if (Number.isFinite(theta) && omega > 0) {
      ctx.strokeStyle = css("--series-2");
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      const steps = 200;
      for (let k = 0; k <= steps; k++) {
        const z = lo + (k / steps) * (hi - lo);
        const w = Math.exp(-((z - theta) ** 2) / (2 * omega * omega));
        const px = X(z), py = Y(w * cmax);
        if (k === 0) ctx.moveTo(px, py); else ctx.lineTo(px, py);
      }
      ctx.stroke();
      ctx.setLineDash([4, 3]);
      ctx.beginPath(); ctx.moveTo(X(theta), pad.t); ctx.lineTo(X(theta), H - pad.b); ctx.stroke();
      ctx.setLineDash([]);
    }
    ctx.strokeStyle = css("--series-1");
    ctx.lineWidth = 1.5;
    ctx.beginPath(); ctx.moveTo(X(mean), pad.t); ctx.lineTo(X(mean), H - pad.b); ctx.stroke();

    ctx.font = "11px " + css("--font");
    ctx.textAlign = "left";
    ctx.textBaseline = "top";
    ctx.fillStyle = css("--series-1"); ctx.fillRect(pad.l + 8, pad.t + 6, 10, 2);
    ctx.fillStyle = css("--ink"); ctx.fillText("phenotypes, mean", pad.l + 22, pad.t + 1);
    ctx.fillStyle = css("--series-2"); ctx.fillRect(pad.l + 140, pad.t + 6, 10, 2);
    ctx.fillStyle = css("--ink"); ctx.fillText("fitness, optimum", pad.l + 154, pad.t + 1);
  }
}

// ------------------------------------------------------------------ state

let worker = null;
let ready = false;
let store = new Store(COLS.length);
let textParts = [];
let header = "";
let fileUsed = "";
let params = null;
let popSize = 0;
let pop = { pheno: null, fit: null, theta: NaN, omega: NaN };
let status = "idle"; // idle | ready | running | paused | done | dead | error
let gen = 0;
let plannedGens = 0;
let stale = false;
let rate = 0;

const charts = {};

function buildCharts() {
  charts.z = new LineChart($("ch-z"), [
    { get: (s) => s.col(1), color: "--series-1", label: "z̄" },
    { get: (s) => s.col(5), color: "--series-2", label: "θ", dash: [4, 3] },
  ], { legend: true });
  charts.vg = new LineChart($("ch-vg"), [{ get: (s) => s.col(3), color: "--series-1", label: "V_G" }], { zero: true });
  let lagCache = { n: -1, a: null };
  charts.lag = new LineChart($("ch-lag"), [{
    get: (s) => {
      if (lagCache.n !== s.n) {
        const a = new Float64Array(s.n), th = s.col(5), z = s.col(1);
        for (let i = 0; i < s.n; i++) a[i] = th[i] - z[i];
        lagCache = { n: s.n, a };
      }
      return lagCache.a;
    },
    color: "--series-1", label: "θ − z̄",
  }], { zero: true });
  let survCache = { n: -1, a: null };
  charts.surv = new LineChart($("ch-surv"), [{
    get: (s) => {
      if (survCache.n !== s.n) {
        const a = new Float64Array(s.n), c = s.col(4);
        for (let i = 0; i < s.n; i++) a[i] = c[i] / (popSize || 1);
        survCache = { n: s.n, a };
      }
      return survCache.a;
    },
    color: "--series-1", label: "survived",
  }], { zero: true });
  charts.pop = new PopChart($("ch-pop"));
}

let pending = false;
function schedule() {
  if (pending) return;
  pending = true;
  requestAnimationFrame(() => { pending = false; render(); });
}

function render() {
  const xmax = plannedGens;
  for (const k of ["z", "vg", "lag", "surv"]) charts[k].draw(store, xmax);
  charts.pop.draw(pop.pheno, pop.theta, pop.omega);
  const z = store.last(1), th = store.last(5), vp = store.last(11);
  $("c-gen").textContent = store.n ? `${gen.toLocaleString()} / ${plannedGens.toLocaleString()}` : "–";
  $("c-z").textContent = fmt(z);
  $("c-theta").textContent = fmt(th);
  $("c-lag").textContent = fmt((th - z) / Math.sqrt(vp));
  $("c-vg").textContent = fmt(store.last(3));
  $("c-surv").textContent = store.n ? `${store.last(4)} / ${popSize}` : "–";
  renderStatus();
}

function renderStatus() {
  const el = $("status");
  el.classList.toggle("dead", status === "dead");
  const words = {
    idle: "Loading…", ready: "Ready", running: "Running", paused: "Paused",
    done: "Finished", dead: `Population died in generation ${gen}`, error: "Not run",
  };
  el.textContent = (words[status] || "") + (stale && status !== "running" ? " · settings changed: Reset to apply" : "");
  $("run").textContent = status === "running" ? "Pause" : "Run";
  $("step").disabled = !(status === "ready" || status === "paused");
  $("speed").textContent = rate && status === "running" ? `${Math.round(rate).toLocaleString()} generations/s` : "";
}

function markStale() {
  if (status !== "idle" && status !== "error" && params) {
    stale = (textEdited ? $("optText").value : optionsText(getForm())) !== lastText;
  }
  renderStatus();
}

let lastText = "";

// ------------------------------------------------------------------ the worker

function showError(msg) {
  const el = $("error");
  el.hidden = !msg;
  el.textContent = msg ? `QTSim does not run these settings:\n${msg}` : "";
}

function newRun(text) {
  if (!ready) return;
  store = new Store(COLS.length);
  textParts = [];
  pop = { pheno: null, fit: null, theta: NaN, omega: NaN };
  gen = 0;
  params = null;
  lastText = text;
  stale = false;
  showError("");
  const g = /^\s*generations\s+(-?\d+)/im.exec(text);
  plannedGens = g ? Math.max(1, Number(g[1])) : 20000;
  worker.postMessage({ type: "new", options: text, mode: mode(), seedLo: (Math.random() * 2 ** 32) >>> 0, seedHi: 0, keepText: true });
}

function currentText() {
  return textEdited ? $("optText").value : optionsText(getForm());
}

function startWorker(wasmBytes) {
  const src = window.QTSIM_WORKER_SRC;
  worker = src ? new Worker(URL.createObjectURL(new Blob([src], { type: "text/javascript" }))) : new Worker("worker.js");
  worker.onmessage = (e) => {
    const m = e.data;
    switch (m.type) {
      case "ready":
        ready = true;
        status = "ready";
        newRun(currentText());
        break;
      case "created":
        header = m.header;
        fileUsed = m.file;
        params = m.params;
        popSize = m.popSize;
        pop.theta = m.params.theta;
        pop.omega = m.params.omega;
        status = m.status === 0 ? (wantRun ? "running" : "ready") : m.status === 2 ? "dead" : "done";
        if (wantRun && m.status === 0) worker.postMessage({ type: "run" });
        wantRun = false;
        schedule();
        break;
      case "batch":
        if (m.records.length) store.push(m.records);
        if (m.text) textParts.push(m.text);
        pop = { pheno: m.pheno, fit: m.fit, theta: m.theta, omega: m.omega };
        gen = m.gen;
        if (m.rate) rate = m.rate;
        schedule();
        break;
      case "ended":
        gen = m.gen;
        status = m.status === 2 ? "dead" : "done";
        schedule();
        break;
      case "error":
        status = "error";
        showError(m.message);
        renderStatus();
        break;
    }
  };
  worker.postMessage({ type: "init", wasm: wasmBytes }, [wasmBytes]);
}

let wantRun = false;

async function loadWasm() {
  if (window.QTSIM_WASM_B64) {
    const s = atob(window.QTSIM_WASM_B64);
    const b = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) b[i] = s.charCodeAt(i);
    return b.buffer;
  }
  const r = await fetch("qtsim.wasm");
  if (!r.ok) throw new Error("qtsim.wasm not found (build it: make web)");
  return r.arrayBuffer();
}

// ------------------------------------------------------------------ controls

function toggleRun() {
  if (!ready) return;
  if (status === "running") {
    worker.postMessage({ type: "pause" });
    status = "paused";
  } else if (status === "ready" || status === "paused") {
    if (stale) { wantRun = true; newRun(currentText()); return; }
    worker.postMessage({ type: "run" });
    status = "running";
  } else {
    wantRun = true;
    newRun(currentText());
  }
  renderStatus();
}

function step() {
  if (status === "ready" || status === "paused") {
    worker.postMessage({ type: "step", n: 1 });
    status = "paused";
    renderStatus();
  }
}

function reset() {
  worker.postMessage({ type: "pause" });
  newRun(currentText());
}

function init() {
  const sel = $("preset");
  PRESETS.forEach((p, i) => sel.add(new Option(p.name, String(i))));
  sel.addEventListener("change", () => {
    textEdited = false;
    setForm({ ...BASE, ...PRESETS[Number(sel.value)].set });
    reset();
  });
  setForm({ ...BASE, ...PRESETS[0].set });

  for (const k of FIELDS) $(k).addEventListener("input", () => { textEdited = false; syncForm(); });
  for (const r of document.querySelectorAll('input[name="mode"]')) r.addEventListener("change", () => { syncForm(); stale = true; renderStatus(); });
  $("reseed").addEventListener("click", () => { $("Seed").value = String(1 + Math.floor(Math.random() * 2 ** 31)); textEdited = false; syncForm(); });
  $("optText").addEventListener("input", () => { textEdited = true; markStale(); });
  $("useText").addEventListener("click", () => { textEdited = true; wantRun = true; newRun($("optText").value); });
  $("openFile").addEventListener("change", async (e) => {
    const f = e.target.files[0];
    if (!f) return;
    const t = await f.text();
    setForm(formFromText(t));
    $("optText").value = t;
    textEdited = true;
    reset();
    e.target.value = "";
  });

  $("run").addEventListener("click", toggleRun);
  $("step").addEventListener("click", step);
  $("reset").addEventListener("click", reset);

  $("dl-csv").addEventListener("click", () => {
    const rows = [COLS.join(",")];
    for (let i = 0; i < store.n; i++) rows.push(COLS.map((_, j) => store.cols[j][i]).join(","));
    download("qtsim_records.csv", rows.join("\n") + "\n", "text/csv");
  });
  $("dl-out").addEventListener("click", () => download("qtsim_output.txt", header + textParts.join("")));
  $("dl-opt").addEventListener("click", () => download("qtsim_options.txt", lastText || currentText()));

  $("theme").addEventListener("click", () => {
    const root = document.documentElement;
    const dark = root.dataset.theme ? root.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = dark ? "light" : "dark";
    try { localStorage.setItem("qtsim-theme", root.dataset.theme); } catch (_) { /* no storage */ }
    schedule();
  });
  try {
    const t = localStorage.getItem("qtsim-theme");
    if (t) document.documentElement.dataset.theme = t;
  } catch (_) { /* no storage */ }

  document.addEventListener("keydown", (e) => {
    if (e.target.closest("input, textarea, select")) return;
    if (e.key === " ") { e.preventDefault(); toggleRun(); }
    else if (e.key === "ArrowRight") step();
    else if (e.key === "r" || e.key === "R") reset();
  });
  window.addEventListener("resize", schedule);
  matchMedia("(prefers-color-scheme: dark)").addEventListener("change", schedule);

  buildCharts();
  schedule();
  loadWasm().then(startWorker).catch((err) => { status = "error"; showError(String(err.message || err)); renderStatus(); });
}

// For the console and for checks: what the page would export
window.qtsimExplorer = {
  outputText: () => header + textParts.join(""),
  optionsText: () => lastText,
  records: () => store.n,
};

init();
