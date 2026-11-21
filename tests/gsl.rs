// The random numbers and statistics against GSL 2.8 as installed (Homebrew, Apple
// arm64): FNV-1a hashes of long sequences of draws, computed by a C program linked
// against libgsl (tests/gsl_reference.c).

use qtsim::options::{fmt_f, parse};
use qtsim::rng::{gaussian, poisson, uniform_int, Mt};

fn fnv(mut h: u64, v: u64) -> u64 {
    for i in 0..8 {
        h ^= (v >> (8 * i)) & 0xff;
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}
const FNV0: u64 = 1_469_598_103_934_665_603;

#[test]
fn poisson_matches_gsl() {
    // (mean, seed, hash of 5000 draws); means above 10 take GSL's gamma and
    // binomial route, which uses tan, exp and log
    let cases = [
        (0.3, 1000, 0x50ff4a1b510511c2u64),
        (3.0, 1001, 0x21dc6f0a29aaff4au64),
        (9.9, 1002, 0xe10914bf226c9a74u64),
        (10.5, 1003, 0x2537888e3aee86feu64),
        (14.0, 1004, 0x680bc3884bc0b352u64),
        (25.3, 1005, 0x70e66ec2ed6c592au64),
        (50.0, 1006, 0x1d0f74ae14edc3b4u64),
        (137.0, 1007, 0x4474249573edcba3u64),
        (1000.0, 1008, 0xbc5ca349dd30dfa7u64),
        (100000.0, 1009, 0xe2a638dbe065c73cu64),
    ];
    let mut bad = Vec::new();
    for (mu, seed, want) in cases {
        let mut r = Mt::new(seed);
        let mut h = FNV0;
        for _ in 0..5000 {
            h = fnv(h, poisson(&mut r, mu) as u64);
        }
        if h != want {
            bad.push(mu);
        }
    }
    assert!(bad.is_empty(), "Poisson draws differ from GSL's for means {:?}", bad);
}

// Bit for bit only with the platform's log (Apple's here): the portable log
// (musl's) differs from Apple's in the last bit for about 1 input in 14
#[cfg(feature = "system-libm")]
#[test]
fn gaussian_matches_gsl() {
    let mut r = Mt::new(77);
    let mut h = FNV0;
    for _ in 0..100_000 {
        h = fnv(h, gaussian(&mut r, 0.37).to_bits());
    }
    assert_eq!(h, 0xa8d5ae8afc3d7842);
}

#[test]
fn uniform_int_and_seed_zero_match_gsl() {
    let mut r = Mt::new(0); // GSL replaces a zero seed by 4357
    let mut h = FNV0;
    for _ in 0..10_000 {
        h = fnv(h, uniform_int(&mut r, 999));
    }
    assert_eq!(h, 0x7d27e2f2dc7c4cb9);
}

#[test]
fn printf_formatting() {
    assert_eq!(fmt_f(0.0078125), "0.007812"); // a tie: round half to even, as printf
    assert_eq!(fmt_f(-1e-9), "-0.000000");
    assert_eq!(fmt_f(f64::NAN), "nan");
    assert_eq!(fmt_f(f64::INFINITY), "inf");
}

#[test]
fn options_errors_are_qtsims() {
    let e = parse("PopSize abc\n", 1).unwrap_err();
    assert_eq!(e.message, "command PopSize: abc is not a valid int");
    let e = parse("Ploidy 3\n", 1).unwrap_err();
    assert_eq!(e.message, "Ploidy must be 1 or 2.\n");
    let e = parse("PopSize 1 2\n", 1).unwrap_err();
    assert_eq!(e.message, "PopSize expecting 2 args but found 3\n");
    let e = parse("Bogus 1\n", 1).unwrap_err();
    assert_eq!(e.message, "Cmd not valid Bogus\n");
    let o = parse("# comment\npopsize 10\r\nSeed 5\nInitFitnessSD 2.5\n", 1).unwrap();
    assert_eq!((o.pop_size, o.seed, o.fitness_sd), (10, 5, 2.5));
}

#[test]
fn analytical_width() {
    let o = parse("PhenotypeVar 0.01\nHeritability 0.5\nMutationScalar 0.001\n", 1).unwrap();
    // ω = h²V_P / √(m(1 − h²)V_P)
    assert!((o.fitness_sd - 0.005 / (0.001f64 * 0.005).sqrt()).abs() < 1e-15);
}
