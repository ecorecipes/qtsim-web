//! The statistics of QTSim's output line.
//!
//! Exact mode: GSL 2.8's `gsl_stats_mean`, `gsl_stats_variance`, `gsl_stats_sd`
//! and the slope of `gsl_fit_linear`, transcribed. GSL accumulates in `long
//! double`; on the reference machine (Apple arm64) and in WebAssembly that is a
//! double, as here. (On x86-64 Linux `long double` has 64-bit mantissas, so QTSim
//! built there prints slightly different statistics; this port follows the
//! double-precision build.) The installed GSL fuses `delta*delta − v` and
//! `dx*dx − m`, `dx*dy − m` (contraction), written as fma here.
//!
//! Fast mode 2: QTSimFast's one-pass sums about the first value.

use crate::mathfn::fma;

/// gsl_stats_mean
pub fn mean(d: &[f64]) -> f64 {
    let mut m = 0.0;
    for (i, &x) in d.iter().enumerate() {
        m += (x - m) / (i + 1) as f64;
    }
    m
}

fn compute_variance(d: &[f64], mean: f64) -> f64 {
    let mut v = 0.0;
    for (i, &x) in d.iter().enumerate() {
        let delta = x - mean;
        v += fma(delta, delta, -v) / (i + 1) as f64;
    }
    v
}

/// gsl_stats_variance_m
pub fn variance_m(d: &[f64], mean: f64) -> f64 {
    let n = d.len();
    compute_variance(d, mean) * (n as f64 / (n.wrapping_sub(1)) as f64)
}

/// gsl_stats_sd_m
pub fn sd_m(d: &[f64], mean: f64) -> f64 {
    let n = d.len();
    (compute_variance(d, mean) * (n as f64 / (n.wrapping_sub(1)) as f64)).sqrt()
}

/// gsl_stats_variance
pub fn variance(d: &[f64]) -> f64 {
    variance_m(d, mean(d))
}

/// gsl_stats_sd
pub fn sd(d: &[f64]) -> f64 {
    sd_m(d, mean(d))
}

/// The slope c1 of gsl_fit_linear(x, y)
pub fn fit_slope(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    let (mut mx, mut my, mut dx2, mut dxdy) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        mx += (x[i] - mx) / (i as f64 + 1.0);
        my += (y[i] - my) / (i as f64 + 1.0);
    }
    for i in 0..n {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        dx2 += fma(dx, dx, -dx2) / (i as f64 + 1.0);
        dxdy += fma(dx, dy, -dxdy) / (i as f64 + 1.0);
    }
    dxdy / dx2
}

/// What a line of output prints, computed in one pass (fast mode 2, QTSimFast's
/// OnePass): (phenotype mean, phenotype variance, genotype variance, survivors'
/// phenotype SD, slope of fitness on the survivors' reflected phenotypes)
pub fn one_pass(pheno: &[f64], geno: &[f64], x: &[f64], y: &[f64], theta: f64) -> (f64, f64, f64, f64, f64) {
    let n = pheno.len() as f64;
    let ns = x.len() as f64;
    let (p0, g0) = (pheno[0], geno[0]);
    let (mut sp, mut spp, mut sg, mut sgg) = (0.0, 0.0, 0.0, 0.0);
    for i in 0..pheno.len() {
        let d = pheno[i] - p0;
        let e = geno[i] - g0;
        sp += d;
        spp += d * d;
        sg += e;
        sgg += e * e;
    }
    let pm = p0 + sp / n;
    let pv = (spp - sp * sp / n) / (n - 1.0);
    let gv = (sgg - sg * sg / n) / (n - 1.0);

    let refl = |v: f64| if v > theta { theta - (v - theta) } else { v };
    let x0 = x[0];
    let r0 = refl(x0);
    let y0 = y[0];
    let (mut sx, mut sxx, mut sr, mut srr, mut sy, mut sry) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..x.len() {
        let d = x[i] - x0;
        let r = refl(x[i]) - r0;
        let e = y[i] - y0;
        sx += d;
        sxx += d * d;
        sr += r;
        srr += r * r;
        sy += e;
        sry += r * e;
    }
    let psd = ((sxx - sx * sx / ns) / (ns - 1.0)).sqrt();
    let slope = (sry - sr * sy / ns) / (srr - sr * sr / ns);
    (pm, pv, gv, psd, slope)
}
