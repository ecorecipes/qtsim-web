//! The transcendental functions QTSim's results depend on.
//!
//! IEEE 754 fixes +, −, ×, ÷, sqrt and fma exactly, but not exp, log or tan: each
//! platform's maths library may differ in the last bit. By default the portable
//! implementations of the `libm` crate (a port of musl's) are used on every
//! target, so a native build and the WebAssembly module give the same bits. With
//! the feature `system-libm` the platform's own library is used instead, as the C
//! QTSim does (for comparisons with it).

#[cfg(not(feature = "system-libm"))]
#[inline]
pub fn exp(x: f64) -> f64 {
    libm::exp(x)
}

#[cfg(not(feature = "system-libm"))]
#[inline]
pub fn log(x: f64) -> f64 {
    libm::log(x)
}

#[cfg(not(feature = "system-libm"))]
#[inline]
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

#[cfg(feature = "system-libm")]
#[inline]
pub fn exp(x: f64) -> f64 {
    x.exp()
}

#[cfg(feature = "system-libm")]
#[inline]
pub fn log(x: f64) -> f64 {
    x.ln()
}

#[cfg(feature = "system-libm")]
#[inline]
pub fn tan(x: f64) -> f64 {
    x.tan()
}

/// a·b + c with one rounding (GSL as installed was compiled with contraction,
/// so some of its expressions are fused multiply-adds; see rng.rs and stats.rs)
#[inline]
pub fn fma(a: f64, b: f64, c: f64) -> f64 {
    a.mul_add(b, c)
}
