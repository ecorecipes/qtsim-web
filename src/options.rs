//! QTSim's options: defaults (SetDefOptions), the options file (PassOptionFile.c,
//! with GenLib.c's tokenizer and validators), derived values (CaclOptParm,
//! NLOptParam, GetAnalyticalFitSD) and the printed header (PrintOptions).
//!
//! Errors carry the exact text QTSim prints on standard output before it exits
//! with status 1.

use std::fmt::Write;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveType {
    Fitness,
    Phenotype,
    Continuous,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub generations: i32,
    pub sample: i32,
    pub pop_size: i32,
    pub no_qtl: i32,
    pub ploidy: i32,
    pub phenotype_var: f64,
    pub heritability: f64,
    pub env_var: f64,
    pub mute_var: f64,
    pub mute_var_scalar: f64,
    /// FitnessPar[0], [1]: the optimum θ and the width ω
    pub fitness_mean: f64,
    pub fitness_sd: f64,
    pub opt_var_target: f64,
    pub change_fitness_gen: i32,
    pub change_fitness_sd: f64,
    pub move_fitness_gen: i32,
    pub move_fitness_sd: f64,
    pub drift: bool,
    pub seed: u64,
    /// false if the file has no Seed line (QTSim then takes the clock and process id)
    pub seed_given: bool,
    pub standardise_phenotype: bool,
    pub move_type: MoveType,
    pub init_fitness_sd: f64,
    pub mutation_rate_per_gamete: f64,
    pub recomb_poisson_mean: f64,
    /// what the options file made QTSim print before its header (the Help command)
    pub pre_output: String,
}

/// QTSim's VERSION
pub const VERSION: f64 = 0.1;
const MUT_RATE_PER_QTL: f64 = 0.0002;
const RECOMB_POISSON_DEFAULT: f64 = 1.0;

const COMMANDS: [&str; 18] = [
    "Help",
    "Sample",
    "PopSize",
    "NoQTL",
    "Ploidy",
    "Drift",
    "MutationScalar",
    "Generations",
    "ChangeFitnessSD",
    "MoveFitnessSD",
    "Seed",
    "PhenotypeVar",
    "Heritability",
    "StandardisePhenotype",
    "OptimumMoveType",
    "InitFitnessSD",
    "MutationRatePerGamete",
    "RecombinationPoissonMean",
];

impl Default for Options {
    /// SetDefOptions (the seed is set by the caller when the file gives none)
    fn default() -> Self {
        Options {
            generations: 20000,
            sample: 1,
            pop_size: 1000,
            no_qtl: 1,
            ploidy: 1,
            phenotype_var: 1.0,
            heritability: 0.5,
            env_var: 0.0,
            mute_var: 0.0,
            mute_var_scalar: 1.0 / 1000.00,
            fitness_mean: 0.0,
            fitness_sd: 0.1,
            opt_var_target: -1.0,
            change_fitness_gen: -1,
            change_fitness_sd: 0.0,
            move_fitness_gen: -1,
            move_fitness_sd: 1.0,
            drift: false,
            seed: 0,
            seed_given: false,
            standardise_phenotype: false,
            move_type: MoveType::Phenotype,
            init_fitness_sd: -1.0,
            mutation_rate_per_gamete: MUT_RATE_PER_QTL,
            recomb_poisson_mean: RECOMB_POISSON_DEFAULT,
            pre_output: String::new(),
        }
    }
}

/// An options file QTSim rejects: `message` is what it prints before exit(1)
#[derive(Clone, Debug)]
pub struct OptError {
    pub message: String,
    /// output printed before the error (Help lines)
    pub pre_output: String,
}

/// C's isspace in the "C" locale
fn is_c_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// GenLib's MakeArgv
fn make_argv(line: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut p = 0;
    loop {
        while p < line.len() && is_c_space(line[p]) {
            p += 1;
        }
        if p >= line.len() {
            break;
        }
        let start = p;
        while p < line.len() && !is_c_space(line[p]) {
            p += 1;
        }
        out.push(&line[start..p]);
    }
    out
}

fn s(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

/// C's atoi: strtol's prefix, saturated to a long, then cast to int
pub fn atoi(b: &[u8]) -> i32 {
    let mut p = 0;
    while p < b.len() && is_c_space(b[p]) {
        p += 1;
    }
    let mut neg = false;
    if p < b.len() && (b[p] == b'+' || b[p] == b'-') {
        neg = b[p] == b'-';
        p += 1;
    }
    let mut v: i128 = 0;
    while p < b.len() && b[p].is_ascii_digit() {
        v = (v * 10 + (b[p] - b'0') as i128).min(i64::MAX as i128 + 1);
        p += 1;
    }
    let v = if neg { -v } else { v };
    let v = v.clamp(i64::MIN as i128, i64::MAX as i128) as i64;
    v as i32
}

/// C's atof (strtod's longest valid prefix; decimal, inf, nan; hexadecimal floats
/// are read as their leading "0")
pub fn atof(b: &[u8]) -> f64 {
    let mut p = 0;
    while p < b.len() && is_c_space(b[p]) {
        p += 1;
    }
    let start = p;
    if p < b.len() && (b[p] == b'+' || b[p] == b'-') {
        p += 1;
    }
    let neg = start < b.len() && b[start] == b'-';
    let rest = &b[p..];
    let lower: Vec<u8> = rest.iter().take(8).map(|c| c.to_ascii_lowercase()).collect();
    if lower.starts_with(b"inf") {
        return if neg { f64::NEG_INFINITY } else { f64::INFINITY };
    }
    if lower.starts_with(b"nan") {
        return if neg { -f64::NAN } else { f64::NAN };
    }
    let mut q = p;
    let mut digits = 0;
    while q < b.len() && b[q].is_ascii_digit() {
        q += 1;
        digits += 1;
    }
    if q < b.len() && b[q] == b'.' {
        q += 1;
        while q < b.len() && b[q].is_ascii_digit() {
            q += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return 0.0;
    }
    if q < b.len() && (b[q] == b'e' || b[q] == b'E') {
        let mut r = q + 1;
        if r < b.len() && (b[r] == b'+' || b[r] == b'-') {
            r += 1;
        }
        if r < b.len() && b[r].is_ascii_digit() {
            while r < b.len() && b[r].is_ascii_digit() {
                r += 1;
            }
            q = r;
        }
    }
    let txt = std::str::from_utf8(&b[start..q]).unwrap_or("0");
    let txt = if txt.ends_with('.') || txt.contains(".e") || txt.contains(".E") {
        // Rust's parser wants a digit after the point
        txt.replacen('.', ".0", 1)
    } else {
        txt.to_string()
    };
    let txt = if txt.starts_with('.') || txt.starts_with("-.") || txt.starts_with("+.") {
        txt.replacen('.', "0.", 1)
    } else {
        txt
    };
    txt.parse::<f64>().unwrap_or(0.0)
}

/// GenLib's IsValidInt
fn is_valid_int(b: &[u8]) -> bool {
    let b = if !b.is_empty() && b[0] == b'-' { &b[1..] } else { b };
    b.iter().all(|c| c.is_ascii_digit())
}

/// GenLib's IsValidDouble
fn is_valid_double(b: &[u8]) -> bool {
    if atof(b) != 0.0 {
        return true;
    }
    let mut p = 0;
    while p < b.len() && is_c_space(b[p]) {
        p += 1;
    }
    if p < b.len() && b[p] == b'-' {
        p += 1;
    }
    let mut point = false;
    while p < b.len() {
        if b[p] == b'.' {
            if point {
                return false;
            }
            point = true;
        } else if !b[p].is_ascii_digit() {
            return false;
        }
        p += 1;
    }
    true
}

/// sscanf("%lu"): strtoul's prefix (a sign is allowed; '-' negates modulo 2^64),
/// or None when there are no digits (the seed is then left as it was)
fn scan_ulong(b: &[u8]) -> Option<u64> {
    let mut p = 0;
    while p < b.len() && is_c_space(b[p]) {
        p += 1;
    }
    let mut neg = false;
    if p < b.len() && (b[p] == b'+' || b[p] == b'-') {
        neg = b[p] == b'-';
        p += 1;
    }
    let mut v: u64 = 0;
    let mut any = false;
    let mut overflow = false;
    while p < b.len() && b[p].is_ascii_digit() {
        any = true;
        match v.checked_mul(10).and_then(|x| x.checked_add((b[p] - b'0') as u64)) {
            Some(x) => v = x,
            None => overflow = true,
        }
        p += 1;
    }
    if !any {
        return None;
    }
    if overflow {
        return Some(u64::MAX);
    }
    Some(if neg { v.wrapping_neg() } else { v })
}

fn ieq(a: &[u8], b: &str) -> bool {
    a.len() == b.len() && a.iter().zip(b.bytes()).all(|(x, y)| x.to_ascii_lowercase() == y.to_ascii_lowercase())
}

/// Read an options file as QTSim does (SetDefOptions, PassOptFile, CaclOptParm,
/// NLOptParam). `default_seed` stands in for GetSeed() when there is no Seed line.
pub fn parse(text: &str, default_seed: u64) -> Result<Options, OptError> {
    let mut o = Options { seed: default_seed, ..Options::default() };
    let mut pre = String::new();

    macro_rules! fail {
        ($($t:tt)*) => { return Err(OptError { message: format!($($t)*), pre_output: pre }) };
    }

    for raw in text.as_bytes().split(|&c| c == b'\n') {
        let line = match raw.iter().position(|&c| c == b'\r' || c == 0) {
            Some(i) => &raw[..i],
            None => raw,
        };
        let argv = make_argv(line);
        if argv.is_empty() || argv[0][0] == b'#' {
            continue;
        }
        let argc = argv.len();
        let cmd = s(argv[0]);

        let Some(which) = COMMANDS.iter().position(|c| ieq(argv[0], c)) else {
            fail!("Cmd not valid {}\n", cmd);
        };
        let name = COMMANDS[which];

        macro_rules! nargs {
            ($n:expr) => {
                if argc != $n {
                    fail!("{} expecting {} args but found {}\n", cmd, $n, argc);
                }
            };
        }
        macro_rules! int_arg {
            ($i:expr) => {{
                if !is_valid_int(argv[$i]) {
                    fail!("command {}: {} is not a valid int", cmd, s(argv[$i]));
                }
                atoi(argv[$i])
            }};
        }
        macro_rules! dbl_arg {
            ($i:expr) => {{
                if !is_valid_double(argv[$i]) {
                    fail!("command {}: {} is not a valid float", cmd, s(argv[$i]));
                }
                atof(argv[$i])
            }};
        }

        match name {
            "Help" => {
                for c in COMMANDS {
                    pre.push_str(c);
                    pre.push('\n');
                }
            }
            "Sample" => {
                nargs!(2);
                o.sample = int_arg!(1);
            }
            "PopSize" => {
                nargs!(2);
                o.pop_size = int_arg!(1);
            }
            "NoQTL" => {
                nargs!(2);
                o.no_qtl = int_arg!(1);
            }
            "Ploidy" => {
                nargs!(2);
                o.ploidy = int_arg!(1);
                if !(o.ploidy == 1 || o.ploidy == 2) {
                    fail!("Ploidy must be 1 or 2.\n");
                }
            }
            "Drift" => {
                nargs!(1);
                o.drift = !o.drift;
            }
            "MutationScalar" => {
                nargs!(2);
                o.mute_var_scalar = dbl_arg!(1);
            }
            "Generations" => {
                nargs!(2);
                o.generations = int_arg!(1);
            }
            "ChangeFitnessSD" => {
                nargs!(3);
                let g = int_arg!(1);
                let v = dbl_arg!(2);
                o.change_fitness_gen = g;
                o.change_fitness_sd = v;
            }
            "MoveFitnessSD" => {
                nargs!(3);
                let g = int_arg!(1);
                let v = dbl_arg!(2);
                o.move_fitness_gen = g;
                o.move_fitness_sd = v;
            }
            "Seed" => {
                nargs!(2);
                if let Some(v) = scan_ulong(argv[1]) {
                    o.seed = v;
                    o.seed_given = true;
                }
            }
            "PhenotypeVar" => {
                nargs!(2);
                o.phenotype_var = dbl_arg!(1);
            }
            "Heritability" => {
                nargs!(2);
                o.heritability = dbl_arg!(1);
            }
            "StandardisePhenotype" => {
                nargs!(1);
                o.standardise_phenotype = !o.standardise_phenotype;
            }
            "OptimumMoveType" => {
                nargs!(2);
                if ieq(argv[1], "fitness") {
                    o.move_type = MoveType::Fitness;
                } else if ieq(argv[1], "phenotype") {
                    o.move_type = MoveType::Phenotype;
                } else if ieq(argv[1], "continuous") {
                    o.move_type = MoveType::Continuous;
                }
            }
            "InitFitnessSD" => {
                nargs!(2);
                o.init_fitness_sd = dbl_arg!(1);
                if o.init_fitness_sd < 0.0 {
                    fail!("InitFitnessSD must be >0\n");
                }
            }
            "MutationRatePerGamete" => {
                nargs!(2);
                o.mutation_rate_per_gamete = dbl_arg!(1);
                if o.mutation_rate_per_gamete < 0.0 {
                    fail!("MutationRatePerGamete must be >0\n");
                }
            }
            "RecombinationPoissonMean" => {
                nargs!(2);
                o.recomb_poisson_mean = dbl_arg!(1);
                if o.recomb_poisson_mean < 0.0 {
                    fail!("RecombPoissonMean must be >0\n");
                }
            }
            _ => unreachable!(),
        }
    }

    // CaclOptParm
    o.opt_var_target = o.heritability * o.phenotype_var;
    o.env_var = o.phenotype_var - o.opt_var_target;
    o.mute_var = o.env_var * o.mute_var_scalar;

    // NLOptParam
    if o.init_fitness_sd != -1.0 {
        o.fitness_sd = o.init_fitness_sd;
    } else {
        o.fitness_sd = analytical_fit_sd(&o);
    }

    o.pre_output = pre;
    Ok(o)
}

/// GetAnalyticalFitSD: ω = h²V_P / √(m·(1 − h²)V_P)
pub fn analytical_fit_sd(o: &Options) -> f64 {
    let num = o.phenotype_var * o.heritability;
    let dom = o.phenotype_var * (1.0 - o.heritability);
    let dom = (o.mute_var_scalar * dom).sqrt();
    num / dom
}

/// printf("%f")
pub fn fmt_f(x: f64) -> String {
    if x.is_nan() { "nan".to_string() } else { format!("{:.6}", x) }
}

impl Options {
    /// PrintOptions
    pub fn header(&self) -> String {
        let mut h = String::new();
        let f = fmt_f;
        let _ = writeln!(h, "Version:\t{}", f(VERSION));
        let _ = writeln!(h, "Fitness:\t{}\t{}", f(self.fitness_mean), f(self.fitness_sd));
        let _ = writeln!(h, "Sample:\t{}", self.sample);
        let _ = writeln!(h, "PopSize:\t{}", self.pop_size);
        let _ = writeln!(h, "NoQTL:\t{}", self.no_qtl);
        let _ = writeln!(h, "Ploidy:\t{}", self.ploidy);
        let _ = writeln!(h, "OptimizeVariance:\t{}", f(self.opt_var_target));
        let _ = writeln!(h, "Drift:\t{}", if self.drift { "True" } else { "False" });
        let _ = writeln!(h, "EnvVar:\t{}", f(self.env_var));
        let _ = writeln!(h, "MutationScalar:\t{}", f(self.mute_var_scalar));
        let _ = writeln!(h, "MutationVar:\t{}", f(self.mute_var));
        let _ = writeln!(h, "Generations:\t{}", self.generations);
        let _ = writeln!(h, "ChangeFitnessSD:\t{}\t{}", self.change_fitness_gen, f(self.change_fitness_sd));
        let _ = writeln!(h, "MoveFitnessSD:\t{}\t{}", self.move_fitness_gen, f(self.move_fitness_sd));
        if self.move_fitness_gen != -1 {
            let t = match self.move_type {
                MoveType::Fitness => "Fitness",
                MoveType::Phenotype => "Phenotype",
                MoveType::Continuous => "Continuous",
            };
            let _ = writeln!(h, "MoveType:\t{}", t);
        }
        let _ = writeln!(h, "Seed:\t{}", self.seed as i64);
        let _ = writeln!(h, "PheotypeVar:\t{}", f(self.phenotype_var));
        let _ = writeln!(h, "Heritabitlity:\t{}", f(self.heritability));
        let _ = writeln!(h, "StandardisePhenotype:\t{}", if self.standardise_phenotype { "True" } else { "False" });
        let _ = writeln!(h, "InitFitnessSD:\t{}", f(self.init_fitness_sd));
        let _ = writeln!(h, "MutationRatePerGamete\t{}", f(self.mutation_rate_per_gamete));
        let _ = writeln!(h, "RecombinationPoissonMean\t{}", f(self.recomb_poisson_mean));
        h
    }

    /// The options as an options file QTSim reads back to the same run
    pub fn to_file(&self) -> String {
        let mut t = String::new();
        let _ = writeln!(t, "Sample {}", self.sample);
        let _ = writeln!(t, "PopSize {}", self.pop_size);
        let _ = writeln!(t, "NoQTL {}", self.no_qtl);
        let _ = writeln!(t, "Ploidy {}", self.ploidy);
        let _ = writeln!(t, "Generations {}", self.generations);
        if self.drift {
            let _ = writeln!(t, "Drift");
        }
        let _ = writeln!(t, "PhenotypeVar {:?}", self.phenotype_var);
        let _ = writeln!(t, "Heritability {:?}", self.heritability);
        let _ = writeln!(t, "MutationScalar {:?}", self.mute_var_scalar);
        let _ = writeln!(t, "MutationRatePerGamete {:?}", self.mutation_rate_per_gamete);
        let _ = writeln!(t, "RecombinationPoissonMean {:?}", self.recomb_poisson_mean);
        if self.init_fitness_sd != -1.0 {
            let _ = writeln!(t, "InitFitnessSD {:?}", self.init_fitness_sd);
        }
        if self.move_fitness_gen != -1 {
            let _ = writeln!(t, "MoveFitnessSD {} {:?}", self.move_fitness_gen, self.move_fitness_sd);
            let m = match self.move_type {
                MoveType::Fitness => "fitness",
                MoveType::Phenotype => "phenotype",
                MoveType::Continuous => "continuous",
            };
            let _ = writeln!(t, "OptimumMoveType {}", m);
        }
        if self.change_fitness_gen != -1 {
            let _ = writeln!(t, "ChangeFitnessSD {} {:?}", self.change_fitness_gen, self.change_fitness_sd);
        }
        if self.standardise_phenotype {
            let _ = writeln!(t, "StandardisePhenotype");
        }
        let _ = writeln!(t, "Seed {}", self.seed);
        t
    }
}
