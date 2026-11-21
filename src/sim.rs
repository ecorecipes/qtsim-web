//! QTSim's simulation (RunSim and the generation it repeats), on QTSimFast's
//! genomes: a chromosome is a list of block numbers, a block holds the alleles of B
//! consecutive loci, and chromosomes share blocks until a crossover inside a block
//! or a mutation makes a new one.
//!
//! Three modes:
//! - `Exact`: QTSim's output byte for byte (same draws in the same order, the same
//!   floating-point operations in the same order; genotypes summed locus by locus);
//! - `Fast1`: genotypes are sums of cached block sums (QTSimFast's QTSIM_FAST=1);
//!   QTSim's draws, genotypes rounded differently (about 1e-16);
//! - `Fast2`: also QTSimFast's QTSIM_FAST=2 random numbers (independent xoshiro
//!   streams per individual and generation) and one-pass statistics; equal to QTSim
//!   in distribution.
//!
//! A generation is, as in QTSim: phenotypes and fitness (moving the optimum),
//! viability survival, offspring of random pairs of distinct survivors (gametes with
//! at most one crossover, at the lowest of a Poisson number of points), an output
//! line for the parents, a change of width if due, mutation of the offspring.

use crate::mathfn::exp;
use crate::options::{fmt_f, MoveType, Options};
use crate::rng::{self, xs_key, xs_mix, Mt, Uniform, Xs, Zig};
use crate::stats;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Exact,
    Fast1,
    Fast2,
}

impl Mode {
    pub fn from_index(i: i32) -> Option<Mode> {
        match i {
            0 => Some(Mode::Exact),
            1 => Some(Mode::Fast1),
            2 => Some(Mode::Fast2),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    Done,
    Dead,
}

/// Values per record: generation, phenotype mean, phenotype ratio (mean/SD),
/// genotype variance, survivors, optimum θ, width ω, surviving slope, standardised
/// slope, w, genotype mean, phenotype variance
pub const RECORD_LEN: usize = 12;

/// The column line QTSim prints after its header
pub const COLUMNS: &str = "Generation\tPhenotype Mean\tPhenotype Ratio\tGenotype Var\tP Survived\tFitness Function Mean\tFitness Function SD\tSurviving Slope\tSurviving Standardised Slope\tw\t\n";

pub struct Sim {
    pub o: Options,
    pub mode: Mode,
    n: usize,
    l: usize,
    p: usize,
    no_qtl: i32,

    // genomes
    b: usize,
    nb: usize,
    last_len: usize,
    row: usize,
    data: Vec<f64>,
    bsum: Vec<f64>,
    hw: u32,
    free: Vec<u32>,
    owner: Vec<u32>,
    new: Vec<u32>,
    ids: Vec<u32>,
    nids: Vec<u32>,
    mark: Vec<u8>,
    gc_every: i32,
    since_gc: i32,

    // gametes of the next population
    src1: Vec<u32>,
    src2: Vec<u32>,
    cut: Vec<i32>,
    off: Vec<i32>,
    points: Vec<i32>,
    seen: Vec<u32>,
    stamp: u32,
    muts: Vec<(u32, i32, f64)>,

    // individuals of the current population
    geno: Vec<f64>,
    pheno: Vec<f64>,
    fit: Vec<f64>,
    env: Vec<f64>,
    nenv: Vec<f64>,
    surv: Vec<usize>,
    xs_buf: Vec<f64>,
    ys_buf: Vec<f64>,

    rng: Mt,
    xs: Xs,
    zig: Zig,
    env_sd: f64,
    rec_mu: f64,
    rec_emu: f64,
    mut_mu: f64,
    mut_emu: f64,
    mut_sd: f64,

    gen_no: i32,
    pub status: Status,
    /// QTSim's standard output since the last take (if kept)
    pub text: String,
    pub keep_text: bool,
    /// RECORD_LEN values per output line since the last take
    pub records: Vec<f64>,
    preamble_text: String,
}

/// QTSimFast's default block size (any size gives the same output)
fn def_block_size(no_qtl: usize) -> usize {
    if no_qtl <= 16 {
        no_qtl.max(1)
    } else if no_qtl < 400 {
        16
    } else if no_qtl < 2500 {
        32
    } else {
        64
    }
}

impl Sim {
    /// The simulation of `o` (as QTSim's main: options already parsed, the width set).
    /// Errors for inputs QTSim does not run sensibly (it crashes or exits on them).
    pub fn new(o: Options, mode: Mode) -> Result<Sim, String> {
        if o.pop_size < 0 || o.no_qtl < 0 {
            return Err("PopSize and NoQTL must not be negative.".into());
        }
        if o.sample == 0 && o.generations > 0 && o.pop_size > 0 {
            return Err("Sample 0 makes QTSim divide by zero.".into());
        }
        let n = o.pop_size as usize;
        let l = if o.no_qtl == 0 { 1 } else { o.no_qtl as usize };
        let p = o.ploidy as usize;
        let b = def_block_size(l).min(l);
        let nb = l.div_ceil(b);
        let last_len = l - (nb - 1) * b;
        let row = n * p;
        if (2 * row * nb + 2) as u64 >= u32::MAX as u64 {
            return Err("population too large".into());
        }

        let rec_mu = o.recomb_poisson_mean;
        let mut_mu = o.no_qtl as f64 * o.mutation_rate_per_gamete;
        let mut_var = o.mute_var_scalar / ((o.ploidy * o.no_qtl) as f64 * o.mutation_rate_per_gamete) * o.env_var;
        let fast = mode != Mode::Exact;

        let mut s = Sim {
            mode,
            n,
            l,
            p,
            no_qtl: o.no_qtl,
            b,
            nb,
            last_len,
            row,
            data: vec![0.0; b],
            bsum: if fast { vec![0.0; 1] } else { Vec::new() },
            hw: 1,
            free: Vec::new(),
            owner: vec![0; 1],
            new: Vec::new(),
            ids: vec![0; row * nb],
            nids: vec![0; row * nb],
            mark: vec![0; 1],
            gc_every: if l < 2500 { 4 } else { 1 },
            since_gc: 0,
            src1: vec![0; row],
            src2: vec![0; row],
            cut: vec![0; row],
            off: vec![-1; row],
            points: Vec::with_capacity(16),
            seen: vec![0; l + 1],
            stamp: 0,
            muts: Vec::new(),
            geno: vec![0.0; n],
            pheno: vec![0.0; n],
            fit: vec![0.0; n],
            env: vec![0.0; n],
            nenv: vec![0.0; n],
            surv: Vec::with_capacity(n),
            xs_buf: Vec::with_capacity(n),
            ys_buf: Vec::with_capacity(n),
            rng: Mt::new(o.seed),
            xs: Xs::from_key(o.seed),
            zig: Zig::new(),
            env_sd: o.env_var.sqrt(),
            rec_mu,
            rec_emu: exp(-rec_mu),
            mut_mu,
            mut_emu: exp(-mut_mu),
            mut_sd: mut_var.sqrt(),
            gen_no: 0,
            status: Status::Running,
            text: String::new(),
            keep_text: true,
            records: Vec::new(),
            preamble_text: String::new(),
            o,
        };

        // main prints the options (PrintOptions), then RunSim sets Opt->Drift = FALSE
        // before its first generation: the Drift option changes the header only
        s.preamble_text = format!("{}{}{}", s.o.pre_output, s.o.header(), COLUMNS);
        s.o.drift = false;

        // RunSim's population of no one
        if n == 0 {
            s.status = if s.o.generations > 0 { Status::Dead } else { Status::Done };
            return Ok(s);
        }

        // CreatePop twice: the environmental deviations of both populations
        let has_env = s.o.env_var != 0.0;
        for i in 0..n {
            s.env[i] = if has_env { s.gaussian_main(s.env_sd) } else { 0.0 };
        }
        for i in 0..n {
            s.nenv[i] = if has_env { s.gaussian_main(s.env_sd) } else { 0.0 };
        }

        if s.o.generations <= 0 {
            s.status = Status::Done;
        }
        Ok(s)
    }

    /// What QTSim prints before the first generation's line: the Help output, the
    /// options header and the column line (as they were when the run was made)
    pub fn preamble(&self) -> String {
        self.preamble_text.clone()
    }

    /// The last line QTSim prints when the population dies
    pub const DIED: &'static str = "Pop all died.\n";

    pub fn generation(&self) -> i32 {
        self.gen_no
    }

    pub fn pheno(&self) -> &[f64] {
        &self.pheno
    }
    pub fn geno(&self) -> &[f64] {
        &self.geno
    }
    pub fn fit(&self) -> &[f64] {
        &self.fit
    }

    #[inline]
    fn gaussian_main(&mut self, sigma: f64) -> f64 {
        match self.mode {
            Mode::Fast2 => sigma * self.xs.normal(&self.zig),
            _ => rng::gaussian(&mut self.rng, sigma),
        }
    }

    #[inline]
    fn block_len(&self, bno: usize) -> usize {
        if bno == self.nb - 1 { self.last_len } else { self.b }
    }

    fn new_block(&mut self, tag: u32) -> u32 {
        let id = if let Some(id) = self.free.pop() {
            id
        } else {
            let id = self.hw;
            self.hw += 1;
            self.data.resize(self.hw as usize * self.b, 0.0);
            self.owner.resize(self.hw as usize, 0);
            self.mark.resize(self.hw as usize, 0);
            if !self.bsum.is_empty() {
                self.bsum.resize(self.hw as usize, 0.0);
            }
            id
        };
        self.owner[id as usize] = tag;
        self.new.push(id);
        id
    }

    #[inline]
    fn set_block_sum(&mut self, id: u32, bno: usize) {
        let len = self.block_len(bno);
        let d = &self.data[id as usize * self.b..id as usize * self.b + len];
        let mut sum = 0.0;
        for &x in d {
            sum += x;
        }
        self.bsum[id as usize] = sum;
    }

    // ------------------------------------------------------------ genotypes

    /// The genotypes of the current population, marking the blocks in use when the
    /// unused ones are to be collected
    fn sum_genotypes(&mut self, marking: bool) {
        let (n, p, row, nb, b) = (self.n, self.p, self.row, self.nb, self.b);
        let geno = &mut self.geno;
        geno.iter_mut().for_each(|g| *g = 0.0);

        if marking {
            for &id in &self.ids {
                self.mark[id as usize] = 1;
            }
        }

        if self.mode != Mode::Exact {
            // cached block sums: Σ_K (sum of chromosome 0's block + chromosome 1's)
            let bs = &self.bsum;
            for k in 0..nb {
                let r = &self.ids[k * row..(k + 1) * row];
                if p == 2 {
                    for i in 0..n {
                        geno[i] += bs[r[2 * i] as usize] + bs[r[2 * i + 1] as usize];
                    }
                } else {
                    for i in 0..n {
                        geno[i] += bs[r[i] as usize];
                    }
                }
            }
            return;
        }

        // locus by locus: G = 0, G += a_l + c_l (diploid) or G += a_l, in locus order;
        // eight individuals side by side (independent chains)
        let data = &self.data;
        for k in 0..nb {
            let len = if k == nb - 1 { self.last_len } else { b };
            let r = &self.ids[k * row..(k + 1) * row];
            let mut i = 0;
            while i + 8 <= n {
                let mut s = [0.0f64; 8];
                s.copy_from_slice(&geno[i..i + 8]);
                if p == 2 {
                    let a: [&[f64]; 8] = std::array::from_fn(|j| {
                        let id = r[2 * (i + j)] as usize;
                        &data[id * b..id * b + len]
                    });
                    let c: [&[f64]; 8] = std::array::from_fn(|j| {
                        let id = r[2 * (i + j) + 1] as usize;
                        &data[id * b..id * b + len]
                    });
                    for x in 0..len {
                        for j in 0..8 {
                            s[j] += a[j][x] + c[j][x];
                        }
                    }
                } else {
                    let a: [&[f64]; 8] = std::array::from_fn(|j| {
                        let id = r[i + j] as usize;
                        &data[id * b..id * b + len]
                    });
                    for x in 0..len {
                        for j in 0..8 {
                            s[j] += a[j][x];
                        }
                    }
                }
                geno[i..i + 8].copy_from_slice(&s);
                i += 8;
            }
            while i < n {
                let mut sum = geno[i];
                if p == 2 {
                    let ia = r[2 * i] as usize;
                    let ic = r[2 * i + 1] as usize;
                    let (a, c) = (&data[ia * b..ia * b + len], &data[ic * b..ic * b + len]);
                    for x in 0..len {
                        sum += a[x] + c[x];
                    }
                } else {
                    let ia = r[i] as usize;
                    for &v in &data[ia * b..ia * b + len] {
                        sum += v;
                    }
                }
                geno[i] = sum;
                i += 1;
            }
        }
    }

    fn collect_blocks(&mut self) {
        // the free blocks: every block below the high-water mark not in use
        let mut new_hw = 0;
        for id in (0..self.hw).rev() {
            if self.mark[id as usize] != 0 {
                new_hw = id + 1;
                break;
            }
        }
        self.free.clear();
        for id in (0..new_hw).rev() {
            if self.mark[id as usize] == 0 {
                self.free.push(id);
            }
        }
        for m in self.mark.iter_mut() {
            *m = 0;
        }
        self.hw = new_hw;
        self.data.truncate(new_hw as usize * self.b);
        self.owner.truncate(new_hw as usize);
        self.mark.truncate(new_hw as usize);
        if !self.bsum.is_empty() {
            self.bsum.truncate(new_hw as usize);
        }
        self.since_gc = 0;
    }

    // ------------------------------------------------------------ one generation

    /// PopFitness: phenotypes, the optimum, fitness
    fn pop_fitness(&mut self, itter: i32) {
        self.since_gc += 1;
        let marking = self.since_gc >= self.gc_every;
        self.sum_genotypes(marking);
        if marking {
            self.collect_blocks();
        }
        for i in 0..self.n {
            self.pheno[i] = self.geno[i] + self.env[i];
        }

        let mut phenotype_sd = 1.0;
        if self.o.standardise_phenotype {
            phenotype_sd = stats::sd(&self.pheno);
        }

        if itter >= self.o.move_fitness_gen && self.o.move_fitness_gen != -1 {
            match self.o.move_type {
                MoveType::Fitness => {
                    self.o.fitness_mean = stats::mean(&self.pheno) + self.o.fitness_sd * self.o.move_fitness_sd
                }
                MoveType::Phenotype => {
                    self.o.fitness_mean = stats::mean(&self.pheno) + self.o.phenotype_var.sqrt() * self.o.move_fitness_sd
                }
                MoveType::Continuous => self.o.fitness_mean += self.o.move_fitness_sd,
            }
        }

        if self.o.drift {
            self.fit.iter_mut().for_each(|f| *f = 1.0);
            return;
        }

        let (theta, omega) = (self.o.fitness_mean, self.o.fitness_sd);
        let scale = 0.4 / omega;
        if !self.o.standardise_phenotype {
            for i in 0..self.n {
                self.fit[i] = rng::gaussian_pdf(self.pheno[i] - theta, omega) / scale;
            }
        } else {
            for i in 0..self.n {
                let ph = (self.pheno[i] - theta) / phenotype_sd;
                self.fit[i] = rng::gaussian_pdf(ph, omega) / scale;
            }
        }
    }

    /// SetSurviving
    fn set_surviving(&mut self, itter: i32) {
        self.surv.clear();
        if self.mode == Mode::Fast2 {
            let seed = self.o.seed;
            for i in 0..self.n {
                let u = (xs_mix(xs_key(seed, itter as u64, 1, i as u64)) >> 11) as f64 * (1.0 / 9007199254740992.0);
                if u < self.fit[i] {
                    self.surv.push(i);
                }
            }
        } else {
            for i in 0..self.n {
                if self.rng.get_double() < self.fit[i] {
                    self.surv.push(i);
                }
            }
        }
    }

    fn dead(&self) -> bool {
        (self.p == 1 && self.surv.is_empty()) || (self.p == 2 && self.surv.len() < 2)
    }

    /// The lowest crossover point of a gamete, or L for none (CreateCombMap; only the
    /// lowest of the points is ever used)
    fn lowest_crossover(&mut self, r: &mut Mt) -> usize {
        let np = rng::poisson_emu(r, self.rec_mu, self.rec_emu) as i32;
        self.lowest_from(r, np, |r, n| rng::uniform_int(r, n as u64) as i32)
    }

    fn lowest_from<R, F: FnMut(&mut R, u32) -> i32>(&mut self, r: &mut R, np: i32, mut draw: F) -> usize {
        if np == 0 || np >= self.no_qtl {
            return self.l;
        }
        let mut low = self.l as i32;
        let m = (self.no_qtl - 1) as u32;
        if np <= 16 {
            self.points.clear();
            for _ in 0..np {
                let pt = loop {
                    let pt = draw(r, m);
                    if !self.points.contains(&pt) {
                        break pt;
                    }
                };
                self.points.push(pt);
                low = low.min(pt);
            }
            return low as usize;
        }
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.seen.iter_mut().for_each(|v| *v = 0);
            self.stamp = 1;
        }
        for _ in 0..np {
            let pt = loop {
                let pt = draw(r, m);
                if self.seen[pt as usize] != self.stamp {
                    break pt;
                }
            };
            self.seen[pt as usize] = self.stamp;
            low = low.min(pt);
        }
        low as usize
    }

    /// Record a gamete: its first `cut` blocks from chromosome src1, the rest from
    /// src2; off: the crossover's place inside block `cut`, or −1
    fn record_gamete(&mut self, g: usize, par: u32, low: usize, second: bool) {
        self.src1[g] = par + second as u32;
        self.src2[g] = par + 1 - second as u32;
        self.off[g] = -1;
        if low == self.l {
            self.cut[g] = self.nb as i32;
            return;
        }
        let bno = low / self.b;
        let off = low - bno * self.b;
        if off == self.block_len(bno) - 1 {
            self.cut[g] = bno as i32 + 1;
            return;
        }
        self.cut[g] = bno as i32;
        self.off[g] = off as i32;
    }

    /// NewPop's draws, exact and Fast1 (QTSim's stream, QTSim's order)
    fn offspring_mt(&mut self) {
        let has_env = self.o.env_var != 0.0;
        let ns = self.surv.len() as u64;
        let mut r = std::mem::replace(&mut self.rng, Mt::new(0));
        for i in 0..self.n {
            if self.p == 1 {
                let p1 = self.surv[rng::uniform_int(&mut r, ns) as usize] as u32;
                self.src1[i] = p1;
                self.src2[i] = p1;
                self.cut[i] = self.nb as i32;
                self.off[i] = -1;
            } else {
                let (mut p1, mut p2);
                loop {
                    p1 = rng::uniform_int(&mut r, ns) as usize;
                    p2 = rng::uniform_int(&mut r, ns) as usize;
                    if p1 != p2 {
                        break;
                    }
                }
                for (g, par) in [(2 * i, self.surv[p1] * 2), (2 * i + 1, self.surv[p2] * 2)] {
                    let low = self.lowest_crossover(&mut r);
                    let second = r.get_double() < 0.5;
                    self.record_gamete(g, par as u32, low, second);
                }
            }
            self.nenv[i] = if has_env { rng::gaussian(&mut r, self.env_sd) } else { 0.0 };
        }
        self.rng = r;
    }

    /// NewPop's draws, Fast2: each offspring's own stream (QTSimFast's OffspringJobX),
    /// mutations recorded
    fn offspring_xs(&mut self, itter: i32) {
        let has_env = self.o.env_var != 0.0;
        let ns = self.surv.len() as u32;
        let seed = self.o.seed;
        self.muts.clear();
        for i in 0..self.n {
            let mut x = Xs::from_key(xs_key(seed, itter as u64, 2, i as u64));
            if self.p == 1 {
                let p1 = self.surv[x.uniform_int(ns) as usize] as u32;
                self.src1[i] = p1;
                self.src2[i] = p1;
                self.cut[i] = self.nb as i32;
                self.off[i] = -1;
            } else {
                let (mut p1, mut p2);
                loop {
                    p1 = x.uniform_int(ns) as usize;
                    p2 = x.uniform_int(ns) as usize;
                    if p1 != p2 {
                        break;
                    }
                }
                for (g, par) in [(2 * i, self.surv[p1] * 2), (2 * i + 1, self.surv[p2] * 2)] {
                    let np = x.poisson(self.rec_mu, self.rec_emu) as i32;
                    let low = self.lowest_from(&mut x, np, |x, n| x.uniform_int(n) as i32);
                    let second = x.uniform53() < 0.5;
                    self.record_gamete(g, par as u32, low, second);
                }
            }
            self.nenv[i] = if has_env { self.env_sd * x.normal(&self.zig) } else { 0.0 };
            for c in 0..self.p {
                let chr = (i * self.p + c) as u32;
                let no = x.poisson(self.mut_mu, self.mut_emu);
                for _ in 0..no {
                    let pos = x.uniform_int(self.no_qtl as u32) as i32;
                    let eff = self.mut_sd * x.normal(&self.zig);
                    self.muts.push((chr, pos, eff));
                }
            }
        }
    }

    /// FillGametes and MixBlocks: the block numbers of the next population, and the
    /// blocks in which a crossover fell
    fn build_gametes(&mut self) {
        let (row, nb, b) = (self.row, self.nb, self.b);
        for k in 0..nb {
            let base = k * row;
            for g in 0..row {
                let src = if (k as i32) < self.cut[g] { self.src1[g] } else { self.src2[g] };
                self.nids[base + g] = self.ids[base + src as usize];
            }
        }
        for g in 0..row {
            let off = self.off[g];
            if off < 0 {
                continue;
            }
            let bno = self.cut[g] as usize;
            let cid = self.ids[bno * row + self.src1[g] as usize];
            let oid = self.ids[bno * row + self.src2[g] as usize];
            if cid == oid {
                continue;
            }
            let id = self.new_block(g as u32 + 1);
            let len = self.block_len(bno);
            for x in 0..len {
                let v = if x as i32 <= off { self.data[cid as usize * b + x] } else { self.data[oid as usize * b + x] };
                self.data[id as usize * b + x] = v;
            }
            if !self.bsum.is_empty() {
                self.set_block_sum(id, bno);
            }
            self.nids[bno * row + g] = id;
        }
    }

    /// Add a mutation's effect to chromosome `chr` of the next population at locus
    /// `pos`, in a block of its own (copied first if shared)
    #[inline]
    fn apply_mutation(&mut self, chr: usize, pos: usize, eff: f64) {
        let bno = pos / self.b;
        let tag = chr as u32 + 1;
        let cell = bno * self.row + chr;
        let mut id = self.nids[cell];
        if self.owner[id as usize] != tag {
            let nid = self.new_block(tag);
            let len = self.block_len(bno);
            let (src, dst) = (id as usize * self.b, nid as usize * self.b);
            self.data.copy_within(src..src + len, dst);
            self.nids[cell] = nid;
            id = nid;
        }
        self.data[id as usize * self.b + pos - bno * self.b] += eff;
        if !self.bsum.is_empty() {
            self.set_block_sum(id, bno);
        }
    }

    /// MutatePop: per chromosome in order, a Poisson number of mutations, each at a
    /// uniform locus with a normal effect (exact and Fast1: drawn here; Fast2: recorded)
    fn mutate(&mut self) {
        if self.mode == Mode::Fast2 {
            let muts = std::mem::take(&mut self.muts);
            for &(chr, pos, eff) in &muts {
                self.apply_mutation(chr as usize, pos as usize, eff);
            }
            self.muts = muts;
        } else {
            for chr in 0..self.row {
                let no = rng::poisson_emu(&mut self.rng, self.mut_mu, self.mut_emu);
                for _ in 0..no {
                    let pos = rng::uniform_int(&mut self.rng, self.no_qtl as u64) as usize;
                    let eff = rng::gaussian(&mut self.rng, self.mut_sd);
                    self.apply_mutation(chr, pos, eff);
                }
            }
        }
        for &id in &self.new {
            self.owner[id as usize] = 0;
        }
        self.new.clear();
    }

    /// Output: the line for the parent generation
    fn output(&mut self, itter: i32) {
        self.xs_buf.clear();
        self.ys_buf.clear();
        for &i in &self.surv {
            self.xs_buf.push(self.pheno[i]);
            self.ys_buf.push(self.fit[i]);
        }
        let theta = self.o.fitness_mean;
        let (pm, pv, gv, psd, slope);
        if self.mode == Mode::Fast2 {
            (pm, pv, gv, psd, slope) = stats::one_pass(&self.pheno, &self.geno, &self.xs_buf, &self.ys_buf, theta);
        } else {
            pm = stats::mean(&self.pheno);
            pv = stats::variance(&self.pheno);
            gv = stats::variance(&self.geno);
            psd = stats::sd(&self.xs_buf);
            for v in self.xs_buf.iter_mut() {
                if *v > theta {
                    *v = theta - (*v - theta);
                }
            }
            slope = stats::fit_slope(&self.xs_buf, &self.ys_buf);
        }
        let ratio = pm / pv.sqrt();
        let w = ((gv * gv) / self.o.mute_var).sqrt();
        let ns = self.surv.len();
        if self.keep_text {
            let f = fmt_f;
            let line = format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t\n",
                itter,
                f(pm),
                f(ratio),
                f(gv),
                ns,
                f(theta),
                f(self.o.fitness_sd),
                f(slope),
                f(slope * psd),
                f(w)
            );
            self.text.push_str(&line);
        }
        let gm = stats::mean(&self.geno);
        self.records.extend_from_slice(&[
            itter as f64,
            pm,
            ratio,
            gv,
            ns as f64,
            theta,
            self.o.fitness_sd,
            slope,
            slope * psd,
            w,
            gm,
            pv,
        ]);
    }

    /// One generation. Returns false if the run had already ended or ends here.
    pub fn step(&mut self) -> bool {
        if self.status != Status::Running {
            return false;
        }
        let itter = self.gen_no;

        self.pop_fitness(itter);

        self.set_surviving(itter);
        if self.dead() {
            self.status = Status::Dead;
            if self.keep_text {
                self.text.push_str(Self::DIED);
            }
            return false;
        }
        if self.mode == Mode::Fast2 {
            self.offspring_xs(itter);
        } else {
            self.offspring_mt();
        }
        self.build_gametes();

        if itter % self.o.sample == 0 {
            self.output(itter);
        }

        if itter == self.o.change_fitness_gen {
            self.o.fitness_sd = self.o.change_fitness_sd;
        }

        self.mutate();

        std::mem::swap(&mut self.ids, &mut self.nids);
        std::mem::swap(&mut self.env, &mut self.nenv);

        self.gen_no += 1;
        if self.gen_no >= self.o.generations {
            self.status = Status::Done;
        }
        true
    }

    /// Up to `max` generations; the number run
    pub fn run(&mut self, max: u32) -> u32 {
        let mut k = 0;
        while k < max && self.step() {
            k += 1;
        }
        k
    }

    /// Blocks in use or free below the high-water mark (for the curious)
    pub fn blocks(&self) -> u32 {
        self.hw
    }
}
