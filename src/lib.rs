//! QTSim, the quantitative-genetic simulation of Pagel, Gardner and Meade (2025,
//! Scientific Reports, doi:10.1038/s41598-025-24445-2), ported to Rust from its C
//! source (QTLSim 1.0, copyright 2023 Andrew Meade, University of Reading,
//! GPL-3.0-or-later) and from QTSimFast, the faster version in QTLSim.jl.
//!
//! This is a modified version of QTSim (GPL section 5): a translation into Rust,
//! with the genome representation of QTSimFast and two optional fast modes. See
//! README.md for what is exact and how it was checked.

pub mod mathfn;
pub mod options;
pub mod rng;
pub mod sim;
pub mod stats;
pub mod wasm;

pub use options::{parse, OptError, Options};
pub use sim::{Mode, Sim, Status};

/// The whole standard output QTSim would print for an options file, and its exit
/// status (for the command-line program and the tests)
pub fn run_to_string(text: &str, default_seed: u64, mode: Mode) -> (String, i32) {
    let o = match parse(text, default_seed) {
        Ok(o) => o,
        Err(e) => return (e.pre_output + &e.message, 1),
    };
    let mut s = match Sim::new(o, mode) {
        Ok(s) => s,
        Err(e) => return (format!("{}\n", e), 1),
    };
    let mut out = s.preamble();
    if s.status == Status::Dead {
        out.push_str(Sim::DIED);
        return (out, 0);
    }
    while s.step() {
        if s.text.len() > 1 << 20 {
            out.push_str(&s.text);
            s.text.clear();
        }
        s.records.clear();
    }
    out.push_str(&s.text);
    (out, 0)
}
