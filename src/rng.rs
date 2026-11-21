//! Random numbers.
//!
//! The exact mode draws as QTSim draws, from GSL 2.8: `gsl_rng_mt19937` seeded by
//! GSL's `mt_set`, and GSL's `gsl_rng_uniform`, `_uniform_pos`, `_uniform_int`,
//! `gsl_ran_gaussian` (polar Box–Muller), `gsl_ran_poisson` (with
//! `gsl_ran_gamma_int` and `gsl_ran_binomial` for means above 10) and
//! `gsl_ran_gaussian_pdf`, transcribed from GSL's source (GPL-3.0-or-later). The
//! GSL that QTSim links against (Homebrew's, on the reference machine) was compiled
//! with floating-point contraction, so some of its expressions are fused
//! multiply-adds; they are written as `fma` here, where clang fuses them (checked
//! against the library: tests/gsl_check.c).
//!
//! The fast mode 2 draws as QTSimFast's QTSIM_FAST=2 (its FastRNG.h): xoshiro256++
//! streams keyed by (seed, generation, phase, individual), 53-bit uniforms, Lemire
//! integers, inversion Poisson for means up to 10, Doornik's ZIGNOR normals.

use crate::mathfn::{exp, fma, log, tan};
use std::f64::consts::PI;

/// A source of 32-bit integers and of doubles in [0, 1), as a `gsl_rng` is.
pub trait Uniform {
    fn get(&mut self) -> u32;
    fn get_double(&mut self) -> f64;
}

// ------------------------------------------------------------------ MT19937

const N: usize = 624;
const M: usize = 397;
const UPPER: u32 = 0x8000_0000;
const LOWER: u32 = 0x7fff_ffff;

/// GSL's `gsl_rng_mt19937`
#[derive(Clone)]
pub struct Mt {
    mt: [u32; N],
    mti: usize,
}

impl Mt {
    /// GSL's `mt_set`: a seed of 0 is replaced by 4357; only its low 32 bits are used.
    pub fn new(seed: u64) -> Mt {
        let s = if seed == 0 { 4357 } else { seed };
        let mut mt = [0u32; N];
        mt[0] = (s & 0xffff_ffff) as u32;
        for i in 1..N {
            mt[i] = 1_812_433_253u32
                .wrapping_mul(mt[i - 1] ^ (mt[i - 1] >> 30))
                .wrapping_add(i as u32);
        }
        Mt { mt, mti: N }
    }

    fn refill(&mut self) {
        let mt = &mut self.mt;
        let magic = |y: u32| if y & 1 != 0 { 0x9908_b0dfu32 } else { 0 };
        for kk in 0..N - M {
            let y = (mt[kk] & UPPER) | (mt[kk + 1] & LOWER);
            mt[kk] = mt[kk + M] ^ (y >> 1) ^ magic(y);
        }
        for kk in N - M..N - 1 {
            let y = (mt[kk] & UPPER) | (mt[kk + 1] & LOWER);
            mt[kk] = mt[kk + M - N] ^ (y >> 1) ^ magic(y);
        }
        let y = (mt[N - 1] & UPPER) | (mt[0] & LOWER);
        mt[N - 1] = mt[M - 1] ^ (y >> 1) ^ magic(y);
        self.mti = 0;
    }
}

impl Uniform for Mt {
    #[inline]
    fn get(&mut self) -> u32 {
        if self.mti >= N {
            self.refill();
        }
        let mut y = self.mt[self.mti];
        self.mti += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    #[inline]
    fn get_double(&mut self) -> f64 {
        self.get() as f64 / 4294967296.0
    }
}

// ------------------------------------------------------------------ GSL's distributions

/// gsl_rng_uniform
#[inline]
pub fn uniform<R: Uniform>(r: &mut R) -> f64 {
    r.get_double()
}

/// gsl_rng_uniform_pos
#[inline]
pub fn uniform_pos<R: Uniform>(r: &mut R) -> f64 {
    loop {
        let x = r.get_double();
        if x != 0.0 {
            return x;
        }
    }
}

/// gsl_rng_uniform_int for a generator with min 0 and max 2^32 − 1; n in 1 ..= 2^32 − 1
/// (GSL calls its error handler, which aborts, for other n; QTSim never asks for them)
#[inline]
pub fn uniform_int<R: Uniform>(r: &mut R, n: u64) -> u64 {
    assert!(n > 0 && n <= 0xffff_ffff, "gsl_rng_uniform_int: invalid n");
    let scale = 0xffff_ffffu64 / n;
    loop {
        let k = r.get() as u64 / scale;
        if k < n {
            return k;
        }
    }
}

/// gsl_ran_gaussian: the polar Box–Muller method (r2 is fused in the library)
pub fn gaussian<R: Uniform>(r: &mut R, sigma: f64) -> f64 {
    loop {
        let x = -1.0 + 2.0 * uniform_pos(r);
        let y = -1.0 + 2.0 * uniform_pos(r);
        let r2 = fma(x, x, y * y);
        if !(r2 > 1.0 || r2 == 0.0) {
            return sigma * y * (-2.0 * log(r2) / r2).sqrt();
        }
    }
}

/// gsl_ran_gaussian_pdf
#[inline]
pub fn gaussian_pdf(x: f64, sigma: f64) -> f64 {
    let u = x / sigma.abs();
    (1.0 / ((2.0 * PI).sqrt() * sigma.abs())) * exp(-u * u / 2.0)
}

/// gsl_ran_poisson(r, mu), with emu = exp(−mu) given for the small-mean method
/// (QTSim's callers compute it once; GSL computes the same value itself)
pub fn poisson_emu<R: Uniform>(r: &mut R, mu: f64, emu: f64) -> u32 {
    if mu > 10.0 {
        return poisson(r, mu);
    }
    let mut prod = 1.0;
    let mut k: u32 = 0;
    loop {
        prod *= uniform(r);
        k = k.wrapping_add(1);
        if !(prod > emu) {
            break;
        }
    }
    k.wrapping_sub(1)
}

/// gsl_ran_poisson
pub fn poisson<R: Uniform>(r: &mut R, mu0: f64) -> u32 {
    let mut mu = mu0;
    let mut k: u32 = 0;
    while mu > 10.0 {
        let m = (mu * (7.0 / 8.0)) as u32;
        let x = gamma_int(r, m);
        if x >= mu {
            return k.wrapping_add(binomial(r, mu / x, m.wrapping_sub(1)));
        } else {
            k = k.wrapping_add(m);
            mu -= x;
        }
    }
    let emu = exp(-mu);
    let mut prod = 1.0;
    loop {
        prod *= uniform(r);
        k = k.wrapping_add(1);
        if !(prod > emu) {
            break;
        }
    }
    k.wrapping_sub(1)
}

/// gsl_ran_gamma_int
fn gamma_int<R: Uniform>(r: &mut R, a: u32) -> f64 {
    if a < 12 {
        let mut prod = 1.0;
        for _ in 0..a {
            prod *= uniform_pos(r);
        }
        -log(prod)
    } else {
        gamma_large(r, a as f64)
    }
}

/// GSL's gamma_large (Knuth, after Ahrens); fused as clang fuses it
fn gamma_large<R: Uniform>(r: &mut R, a: f64) -> f64 {
    let sqa = fma(2.0, a, -1.0).sqrt();
    loop {
        let (mut x, mut y);
        loop {
            y = tan(PI * uniform(r));
            x = fma(sqa, y, a) - 1.0;
            if !(x <= 0.0) {
                break;
            }
        }
        let v = uniform(r);
        if !(v > fma(y, y, 1.0) * exp(fma(a - 1.0, log(x / (a - 1.0)), -(sqa * y)))) {
            return x;
        }
    }
}

/// gsl_pow_uint
fn pow_uint(mut x: f64, mut n: u32) -> f64 {
    let mut value = 1.0;
    loop {
        if n & 1 != 0 {
            value *= x;
        }
        n >>= 1;
        x *= x;
        if n == 0 {
            return value;
        }
    }
}

fn stirling(y1: f64) -> f64 {
    let y2 = y1 * y1;
    (13860.0 - (462.0 - (132.0 - (99.0 - 140.0 / y2) / y2) / y2) / y2) / y1 / 166320.0
}

/// gsl_ran_binomial (the TPE algorithm of Kachitvichyanukul and Schmeiser, as GSL
/// writes it: squeeze, Stirling corrections); fused as clang fuses it
fn binomial<R: Uniform>(r: &mut R, p0: f64, n: u32) -> u32 {
    const SMALL_MEAN: f64 = 14.0;
    const BINV_CUTOFF: i32 = 110;
    const FAR_FROM_MEAN: i32 = 20;

    if n == 0 {
        return 0;
    }
    let mut p = p0;
    let mut flipped = false;
    if p > 0.5 {
        p = 1.0 - p;
        flipped = true;
    }
    let q = 1.0 - p;
    let s = p / q;
    let np = n as f64 * p;
    let ix: i32;

    if np < SMALL_MEAN {
        let f0 = pow_uint(q, n);
        'outer: loop {
            let mut f = f0;
            let mut u = uniform(r);
            let mut i = 0i32;
            while i <= BINV_CUTOFF {
                if u < f {
                    ix = i;
                    break 'outer;
                }
                u -= f;
                f *= s * (n as f64 - i as f64) / (i + 1) as f64;
                i += 1;
            }
        }
    } else {
        let ffm = np + p;
        let m = ffm as i32;
        let fm = m as f64;
        let xm = fm + 0.5;
        let npq = np * q;
        let p1 = (fma(2.195, npq.sqrt(), -(4.6 * q))).floor() + 0.5;
        let xl = xm - p1;
        let xr = xm + p1;
        let c = 0.134 + 20.5 / (15.3 + fm);
        let p2 = p1 * (1.0 + c + c);
        let al = (ffm - xl) / fma(-xl, p, ffm);
        let lambda_l = al * fma(0.5, al, 1.0);
        let ar = (xr - ffm) / (xr * q);
        let lambda_r = ar * fma(0.5, ar, 1.0);
        let p3 = p2 + c / lambda_l;
        let p4 = p3 + c / lambda_r;

        ix = loop {
            let u = uniform(r) * p4;
            let mut v = uniform(r);
            let cand: i32;
            if u <= p1 {
                break (fma(-p1, v, xm) + u) as i32;
            } else if u <= p2 {
                let x = xl + (u - p1) / c;
                v = fma(v, c, 1.0) - (x - xm).abs() / p1;
                if v > 1.0 || v <= 0.0 {
                    continue;
                }
                cand = x as i32;
            } else if u <= p3 {
                cand = (xl + log(v) / lambda_l) as i32;
                if cand < 0 {
                    continue;
                }
                v *= (u - p2) * lambda_l;
            } else {
                cand = (xr - log(v) / lambda_r) as i32;
                if cand as f64 > n as f64 {
                    continue;
                }
                v *= (u - p3) * lambda_r;
            }

            let k = (cand - m).abs();
            let var;
            let accept;
            if k <= FAR_FROM_MEAN {
                let g = (n as f64 + 1.0) * s;
                let mut f = 1.0;
                var = v;
                if m < cand {
                    for i in m + 1..=cand {
                        f *= g / i as f64 - s;
                    }
                } else if m > cand {
                    for i in cand + 1..=m {
                        f /= g / i as f64 - s;
                    }
                }
                accept = f;
            } else {
                var = log(v);
                if (k as f64) < npq / 2.0 - 1.0 {
                    let kf = k as f64;
                    let amaxp = kf / npq * (fma(kf, kf / 3.0 + 0.625, 1.0 / 6.0) / npq + 0.5);
                    let ynorm = -((k * k) as f64 / (2.0 * npq));
                    if var < ynorm - amaxp {
                        break cand;
                    }
                    if var > ynorm + amaxp {
                        continue;
                    }
                }
                let x1 = cand as f64 + 1.0;
                let w1 = n as f64 - cand as f64 + 1.0;
                let f1 = fm + 1.0;
                let z1 = n as f64 + 1.0 - fm;
                let nm = (n.wrapping_sub(m as u32)) as f64 + 0.5;
                let first = fma(xm, log(f1 / x1), nm * log(z1 / w1));
                let second = fma((cand - m) as f64, log(w1 * p / (x1 * q)), first);
                accept = second + stirling(f1) + stirling(z1) - stirling(x1) - stirling(w1);
            }
            if var <= accept {
                break cand;
            }
        };
    }

    if flipped { n.wrapping_sub(ix as u32) } else { ix as u32 }
}

// ------------------------------------------------------------------ xoshiro256++ (fast mode 2)

/// A xoshiro256++ stream (Blackman and Vigna 2018), as QTSimFast's FAST_XS
#[derive(Clone)]
pub struct Xs {
    s: [u64; 4],
}

/// SplitMix64's output function
#[inline]
pub fn xs_mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// The key of stream (seed, generation, phase, individual)
#[inline]
pub fn xs_key(seed: u64, generation: u64, phase: u64, ind: u64) -> u64 {
    xs_mix(xs_mix(xs_mix(seed.wrapping_add(0x632b_e59b_d9b4_e019)).wrapping_add(generation)).wrapping_add((phase << 40) ^ ind))
}

impl Xs {
    /// A stream seeded by SplitMix64 from a key (also how the run's own stream is
    /// seeded from the options file's seed)
    #[inline]
    pub fn from_key(mut key: u64) -> Xs {
        let mut s = [0u64; 4];
        for v in s.iter_mut() {
            key = key.wrapping_add(0x9e37_79b9_7f4a_7c15);
            *v = xs_mix(key);
        }
        Xs { s }
    }

    #[inline]
    pub fn next(&mut self) -> u64 {
        let s = &mut self.s;
        let result = (s[0].wrapping_add(s[3])).rotate_left(23).wrapping_add(s[0]);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// (0, 1), 53 random bits
    #[inline]
    pub fn open(&mut self) -> f64 {
        ((self.next() >> 11) as f64 + 0.5) * (1.0 / 9007199254740992.0)
    }

    /// [0, 1), 53 random bits
    #[inline]
    pub fn uniform53(&mut self) -> f64 {
        (self.next() >> 11) as f64 * (1.0 / 9007199254740992.0)
    }

    /// Lemire's method: 0 .. n−1, 0 < n < 2^32
    #[inline]
    pub fn uniform_int(&mut self, n: u32) -> u32 {
        let mut m = (self.next() >> 32) * n as u64;
        let mut l = m as u32;
        if l < n {
            let t = n.wrapping_neg() % n;
            while l < t {
                m = (self.next() >> 32) * n as u64;
                l = m as u32;
            }
        }
        (m >> 32) as u32
    }

    /// ZIGNOR (Doornik 2005), 128 layers
    pub fn normal(&mut self, z: &Zig) -> f64 {
        loop {
            let b = self.next();
            let i = (b & 0x7f) as usize;
            let u = 2.0 * ((b >> 11) as f64 * (1.0 / 9007199254740992.0)) - 1.0;
            if u.abs() < z.ratio[i] {
                return u * z.x[i];
            }
            if i == 0 {
                return self.zig_tail(u < 0.0);
            }
            let x = u * z.x[i];
            let f0 = exp(-0.5 * (z.x[i] * z.x[i] - x * x));
            let f1 = exp(-0.5 * (z.x[i + 1] * z.x[i + 1] - x * x));
            if f1 + self.uniform53() * (f0 - f1) < 1.0 {
                return x;
            }
        }
    }

    fn zig_tail(&mut self, negative: bool) -> f64 {
        loop {
            let x = log(self.open()) / ZIG_R;
            let y = log(self.open());
            if !(-2.0 * y < x * x) {
                return if negative { x - ZIG_R } else { ZIG_R - x };
            }
        }
    }

    /// Poisson by inversion, mu <= 10, emu = exp(−mu)
    #[inline]
    pub fn poisson_small(&mut self, mu: f64, emu: f64) -> u32 {
        let u = self.uniform53();
        let mut p = emu;
        let mut f = p;
        let mut k = 0u32;
        while u >= f && k < 1000 {
            k += 1;
            p *= mu / k as f64;
            f += p;
        }
        k
    }

    /// QTSimFast's PoissonX: GSL's algorithm on this stream above 10, inversion below
    #[inline]
    pub fn poisson(&mut self, mu: f64, emu: f64) -> u32 {
        if mu > 10.0 {
            return poisson(self, mu);
        }
        self.poisson_small(mu, emu)
    }
}

/// As a gsl_rng (QTSimFast's TypeGetX, TypeGetDoubleX): for GSL's algorithms on the stream
impl Uniform for Xs {
    #[inline]
    fn get(&mut self) -> u32 {
        (self.next() >> 32) as u32
    }
    #[inline]
    fn get_double(&mut self) -> f64 {
        self.uniform53()
    }
}

pub const ZIG_C: usize = 128;
pub const ZIG_R: f64 = 3.442619855899;
pub const ZIG_V: f64 = 9.91256303526217e-3;

/// ZIGNOR's tables
pub struct Zig {
    pub x: [f64; ZIG_C + 1],
    pub ratio: [f64; ZIG_C],
}

impl Zig {
    pub fn new() -> Zig {
        let mut x = [0.0; ZIG_C + 1];
        let mut ratio = [0.0; ZIG_C];
        let mut f = exp(-0.5 * ZIG_R * ZIG_R);
        x[0] = ZIG_V / f;
        x[1] = ZIG_R;
        x[ZIG_C] = 0.0;
        for i in 2..ZIG_C {
            x[i] = (-2.0 * log(ZIG_V / x[i - 1] + f)).sqrt();
            f = exp(-0.5 * x[i] * x[i]);
        }
        for i in 0..ZIG_C {
            ratio[i] = x[i + 1] / x[i];
        }
        Zig { x, ratio }
    }
}

impl Default for Zig {
    fn default() -> Self {
        Zig::new()
    }
}
