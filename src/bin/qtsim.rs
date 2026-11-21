//! The command-line program: the same interface and output as QTSim's.
//!
//!     qtsim options.txt                 exact (QTSim's output byte for byte)
//!     QTSIM_FAST=1 qtsim options.txt    genotypes from cached block sums
//!     QTSIM_FAST=2 qtsim options.txt    also other random numbers
//!     qtsim --mode fast2 options.txt    the same as QTSIM_FAST=2

use qtsim::{parse, Mode, Sim, Status};
use std::io::Write;

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut mode = match std::env::var("QTSIM_FAST").ok().as_deref() {
        Some("1") => Mode::Fast1,
        Some("2") => Mode::Fast2,
        _ => Mode::Exact,
    };
    if args.len() == 3 && args[0] == "--mode" {
        mode = match args[1].as_str() {
            "exact" | "0" => Mode::Exact,
            "fast1" | "1" => Mode::Fast1,
            "fast2" | "2" => Mode::Fast2,
            m => {
                eprintln!("qtsim: unknown mode {} (exact, fast1, fast2)", m);
                std::process::exit(2);
            }
        };
        args.drain(..2);
    }
    if args.len() != 1 {
        // QTSim prints its own source file's name here
        println!("QTSim.c requires a configuration file.");
        std::process::exit(1);
    }
    let text = match std::fs::read(&args[0]) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => {
            eprintln!("qtsim: cannot read {}: {}", args[0], e);
            std::process::exit(1);
        }
    };

    // GetSeed(): the clock and the process id, when the file gives no seed
    let clock = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let default_seed = (clock as u32 as u64) ^ (std::process::id() as u64).reverse_bits();

    if mode != Mode::Exact {
        eprintln!(
            "qtsim: {}: the output is NOT bit-identical to QTSim's",
            if mode == Mode::Fast1 { "fast mode 1 (cached block sums)" } else { "fast mode 2 (other random numbers)" }
        );
    }

    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::with_capacity(1 << 16, stdout.lock());

    let o = match parse(&text, default_seed) {
        Ok(o) => o,
        Err(e) => {
            let _ = out.write_all(e.pre_output.as_bytes());
            let _ = out.write_all(e.message.as_bytes());
            let _ = out.flush();
            std::process::exit(1);
        }
    };
    let mut s = match Sim::new(o, mode) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("qtsim: {}", e);
            std::process::exit(1);
        }
    };
    let _ = out.write_all(s.preamble().as_bytes());
    if s.status == Status::Dead {
        let _ = out.write_all(Sim::DIED.as_bytes());
    }
    while s.step() {
        if s.text.len() > 1 << 16 {
            let _ = out.write_all(s.text.as_bytes());
            s.text.clear();
        }
        s.records.clear();
    }
    let _ = out.write_all(s.text.as_bytes());
    let _ = out.flush();
}
