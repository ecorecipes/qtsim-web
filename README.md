# QTSim in Rust and WebAssembly, with a browser explorer

**Try it:** <https://ecorecipes.github.io/qtsim-web/> (the explorer, one self-contained
page; it runs in your browser and sends nothing anywhere).

**This is a modified version of QTSim** (its source calls itself QTLSim 1.0), the
quantitative-genetic simulation of Pagel, Gardner and Meade, *Quantitative genetic
analysis of directional adaptation suggests low maximum sustainable rates of change in
agreement with data from field populations*, Scientific Reports (2025),
doi:10.1038/s41598-025-24445-2. QTSim is by Andrew Meade, School of Biological
Sciences, University of Reading, copyright 2023, and is free software under the GNU
General Public License, version 3 or (at your option) any later version. This
repository translates it into Rust (November 2026), with the genome representation of
QTSimFast (the faster C version in [QTLSim.jl](https://github.com/ecorecipes/QTLSim.jl))
and two optional fast modes, compiles it to WebAssembly, and wraps it in a single-page
dashboard. It is distributed under the same licence (`LICENSE`). GSL's algorithms that
QTSim calls are transcribed from GSL 2.8 (GPL-3.0-or-later).

## What is here

- `src/`: the simulation as a Rust library (`sim.rs`: the generation loop on shared blocks
  of loci; `rng.rs`: MT19937 and GSL's distributions, and the fast mode's xoshiro streams;
  `stats.rs`: GSL's statistics; `options.rs`: QTSim's options file, defaults and header;
  `wasm.rs`: the WebAssembly interface).
- `src/bin/qtsim.rs`: a command-line program with QTSim's interface and output.
- `web/`: the explorer (`index.html`, `style.css`, `app.js`; the simulation runs in
  `worker.js`, a Web Worker). `make web` also writes `dist/qtsim.html`, one
  self-contained file (about 280 KB) that opens from disk without a server.
- `tools/`: parity checks against the C programs, the Node runner of the WebAssembly
  module, the bundler. `tests/`: unit tests against GSL, inputs and recorded outputs.

## Build and run

Needs Rust (with `rustup target add wasm32-unknown-unknown`) and Node (for the bundle and
the WebAssembly checks). No other dependency: the crate uses only `libm`.

```sh
make                         # target/release/qtsim
target/release/qtsim tests/inputs/examples/design_0100_loci.txt > out.tsv
make web                     # web/qtsim.wasm and dist/qtsim.html
open dist/qtsim.html         # or: make serve, then http://localhost:8000
make test                    # unit tests; native and WebAssembly outputs against tests/expected.tsv
make pages                   # rebuild dist/qtsim.html and publish it to the gh-pages branch
```

Every push to `main` does the same on GitHub (`.github/workflows/pages.yml`): it runs
`make test` (on Linux x86-64, so the recorded outputs also check that results do not
depend on the platform), builds `dist/qtsim.html` and replaces the `gh-pages` branch, which
GitHub Pages serves. Nothing is published if a test fails. `make pages` is the same by hand.

## The explorer

Settings on the left (population, genetics, phenotype and selection, environmental change,
engine), presets taken from the published design, run, pause, step and reset; live charts
of the mean phenotype against the optimum, the genetic variance, the lag and survival, and
the current generation's phenotypes under the fitness function. The settings write a QTSim
options file, which can be edited, opened from disk and exported, with the records (CSV)
and QTSim's text output. Keyboard: space runs or pauses, → steps, R resets.

## Three modes

| mode | what it does | output |
|:--|:--|:--|
| exact | QTSim's draws in QTSim's order and its arithmetic, operation for operation | QTSim's, byte for byte (see "Exactness") |
| fast 1 | genotypes from a cached sum per block of loci (QTSimFast's `QTSIM_FAST=1`) | the same draws; genotypes rounded differently (about 1e-16); output still identical in every case tested |
| fast 2 | also independent xoshiro256++ streams per individual and generation, ziggurat normals, inversion Poisson, Lemire integers, one-pass statistics (QTSimFast's `QTSIM_FAST=2`) | equal to QTSim in distribution; byte-identical to the C QTSimFast's level 2 |

The command-line program selects them with `QTSIM_FAST=1` or `2`, or `--mode fast1|fast2`;
the explorer with its Engine switch. QTSimFast's fast modes were validated statistically
against the exact program in QTLSim.jl (`validate/qtsimfast_fast*`: 31 summary
statistics, 7 scenarios, Kolmogorov–Smirnov tests on their empirical distributions); the
port's fast modes produce the C program's bytes, so that validation carries over.

## Exactness

The exact mode transcribes QTSim (and QTSimFast's equivalent genome code) and the GSL 2.8
routines QTSim calls: `gsl_rng_mt19937` with GSL's seeding, `gsl_rng_uniform`,
`_uniform_pos`, `_uniform_int`, `gsl_ran_gaussian` (polar Box–Muller), `gsl_ran_poisson`
(with `gsl_ran_gamma_int` and the binomial TPE sampler for means above 10),
`gsl_ran_gaussian_pdf`, `gsl_stats_mean`, `_variance`, `_sd` and the slope of
`gsl_fit_linear`. Two details matter for bit parity:

- **Contraction.** The GSL QTSim links against (Homebrew's, Apple arm64) was compiled
  with fused multiply-adds: `x*x + y*y` in the Gaussian sampler, `delta*delta − v` in the
  variance, and so on are single roundings. Checked against the library: of 2 million
  Gaussians the unfused expression differs in 13%, the fused one in none. They are
  written as `fma` here, where clang fuses them.
- **`long double`.** GSL accumulates statistics in `long double`, which is a double on
  Apple arm64 and in WebAssembly, as here. QTSim built on x86-64 Linux (80-bit `long
  double`) prints slightly different statistics; this port follows the double build.

What IEEE 754 does not fix is `exp`, `log` and `tan`. The C QTSim calls the platform's
(Apple's here, which is not correctly rounded either: it differs from the exact result
for about 1 input in 1000); a browser has no access to it. This port uses one portable
implementation (the `libm` crate, a port of musl's) on every target, so the native
program and the WebAssembly module always agree with each other; musl's results differ
from Apple's in the last bit for about 1 input in 10 to 14. Such a difference changes the
printed output only if it moves a survival draw across its threshold or a printed digit
across a rounding boundary. Building with `--features system-libm` uses the platform's
library instead, which makes the native program reproduce the C program exactly.

## Validation

- **Logic** (native, `--features system-libm`, against the C programs, `tools/parity.sh`):
  456 options files (QTSimFast's parity matrix: a synthetic design matrix of 384 files and
  the paper's 72 published inputs, 48 of them to extinction), all three modes:
  **456 of 456 byte-identical** to QTSim (exact) and to QTSimFast (fast 1, fast 2). The
  one difference found on the way, QTSim ignoring its `Drift` option (it resets it before
  the first generation), was fixed.
- **Portable maths** (the default build, as in the browser): the same 456 files, all three modes: **456 of 456 byte-identical** to the C programs, the published inputs included, despite the last-bit differences of `exp` and `log`: none moved a survival draw or a printed digit in these runs. It can happen; it was not observed.
- **WebAssembly** (the module under Node against the native default build,
  `tools/parity_wasm.sh`): 456 files, all three modes: **456 of 456 identical**. The browser therefore prints what the native program prints, and here what QTSim prints.
- **In a browser** (Chrome, the single-file app opened from disk): the published-design
  preset (100 loci, 12,000 generations) gives output whose SHA-256 equals the C QTSim's
  (exact and fast 1) and the C QTSimFast level 2's (fast 2); 5.1, 4.7 and 2.65 s.
- **Unit tests** (`cargo test`): Poisson draws for means from 0.3 to 100,000 (the large-mean
  route through gamma and binomial variates), Gaussians and integers against hashes of
  GSL's own draws (`tests/gsl_reference.c`); QTSim's option errors; printf formatting.

## The WebAssembly interface

Plain C-ABI functions, no bindings generator (`src/wasm.rs`): `qt_alloc`/`qt_dealloc`
for the options text, `qt_new(ptr, len, mode, seed_lo, seed_hi)` (null on error;
`qt_error_ptr/len` hold QTSim's message), `qt_step(h, n)`, `qt_status(h)` (0 running, 1
finished, 2 died), `qt_records_ptr/len/clear` (12 doubles per output line: generation,
phenotype mean, phenotype ratio, genotype variance, survivors, optimum, width, surviving
slope, standardised slope, w, genotype mean, phenotype variance), `qt_text_ptr/len/clear`
(QTSim's text output), `qt_header_ptr/len`, `qt_pop_ptr(h, 0|1|2)` (phenotypes,
genotypes, fitnesses), `qt_param`, `qt_free`. `tools/run_wasm.mjs` is a complete
example.

## Limits

- Bit parity is with QTSim built against GSL 2.8 with contraction on Apple arm64; other
  builds of QTSim differ from each other at the same level (`exp`, `log`, `long double`).
- Options QTSim mishandles are rejected with a message instead (negative sizes; `Sample
  0`, on which QTSim divides by zero); hexadecimal numbers in options files are read as
  their leading `0`.
- Without a `Seed` line QTSim seeds from the clock and process id; the explorer always
  writes one.
- The browser runs one thread; fast 2's threading in the C version is not used.
