//! The WebAssembly interface: plain C-ABI functions, no bindings generator.
//!
//! JavaScript writes an options file (UTF-8) into memory from `qt_alloc`, makes a
//! run with `qt_new`, advances it with `qt_step` and reads what it produced from
//! memory: records (RECORD_LEN doubles per output line, see sim.rs), QTSim's text
//! output, and the current population's phenotypes, genotypes and fitnesses.
//! Pointers returned by the accessors stay valid until the next call that changes
//! the run (qt_step, the clear functions, qt_free).

use crate::sim::{Mode, Sim, Status, RECORD_LEN};
use std::cell::RefCell;

thread_local! {
    static ERROR: RefCell<String> = const { RefCell::new(String::new()) };
}

pub struct Handle {
    sim: Sim,
    header: String,
    file: String,
}

#[unsafe(no_mangle)]
pub extern "C" fn qt_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `p` and `len` must come from one call of `qt_alloc`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_dealloc(p: *mut u8, len: usize) {
    unsafe { drop(Vec::from_raw_parts(p, 0, len.max(1))) };
}

/// The message of the last failed `qt_new` (what QTSim would print), as UTF-8
#[unsafe(no_mangle)]
pub extern "C" fn qt_error_len() -> usize {
    ERROR.with(|e| e.borrow().len())
}

#[unsafe(no_mangle)]
pub extern "C" fn qt_error_ptr() -> *const u8 {
    ERROR.with(|e| e.borrow().as_ptr())
}

fn set_error(m: String) {
    ERROR.with(|e| *e.borrow_mut() = m);
}

/// A run of the options file at (ptr, len); mode 0 exact, 1 fast 1, 2 fast 2; the
/// seed (hi·2^32 + lo) is used when the file has no Seed line. Null on error.
///
/// # Safety
/// (ptr, len) must be readable memory.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_new(ptr: *const u8, len: usize, mode: i32, seed_lo: u32, seed_hi: u32) -> *mut Handle {
    let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
    let text = String::from_utf8_lossy(bytes);
    let Some(mode) = Mode::from_index(mode) else {
        set_error("unknown mode".into());
        return std::ptr::null_mut();
    };
    let seed = ((seed_hi as u64) << 32) | seed_lo as u64;
    let o = match crate::parse(&text, seed) {
        Ok(o) => o,
        Err(e) => {
            set_error(e.pre_output + &e.message);
            return std::ptr::null_mut();
        }
    };
    let file = o.to_file();
    let mut sim = match Sim::new(o, mode) {
        Ok(s) => s,
        Err(e) => {
            set_error(e);
            return std::ptr::null_mut();
        }
    };
    let header = sim.preamble();
    if sim.status == Status::Dead {
        sim.text.push_str(Sim::DIED);
    }
    Box::into_raw(Box::new(Handle { sim, header, file }))
}

macro_rules! handle {
    ($h:expr) => {
        unsafe { &mut *$h }
    };
}

/// # Safety
/// `h` must come from `qt_new` and not have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_free(h: *mut Handle) {
    if !h.is_null() {
        unsafe { drop(Box::from_raw(h)) };
    }
}

/// Run up to `max` generations; the number run
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_step(h: *mut Handle, max: u32) -> u32 {
    handle!(h).sim.run(max)
}

/// 0 running, 1 finished, 2 died
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_status(h: *mut Handle) -> i32 {
    match handle!(h).sim.status {
        Status::Running => 0,
        Status::Done => 1,
        Status::Dead => 2,
    }
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_generation(h: *mut Handle) -> i32 {
    handle!(h).sim.generation()
}

/// Keep QTSim's text output (on by default) or only the records
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_keep_text(h: *mut Handle, keep: i32) {
    handle!(h).sim.keep_text = keep != 0;
}

#[unsafe(no_mangle)]
pub extern "C" fn qt_record_len() -> usize {
    RECORD_LEN
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_records_ptr(h: *mut Handle) -> *const f64 {
    handle!(h).sim.records.as_ptr()
}

/// The number of doubles (RECORD_LEN per record)
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_records_len(h: *mut Handle) -> usize {
    handle!(h).sim.records.len()
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_records_clear(h: *mut Handle) {
    handle!(h).sim.records.clear();
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_text_ptr(h: *mut Handle) -> *const u8 {
    handle!(h).sim.text.as_ptr()
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_text_len(h: *mut Handle) -> usize {
    handle!(h).sim.text.len()
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_text_clear(h: *mut Handle) {
    handle!(h).sim.text.clear();
}

/// What QTSim prints before the first line (Help output, header, column line)
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_header_ptr(h: *mut Handle) -> *const u8 {
    handle!(h).header.as_ptr()
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_header_len(h: *mut Handle) -> usize {
    handle!(h).header.len()
}

/// The run's options as an options file (with the seed used)
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_file_ptr(h: *mut Handle) -> *const u8 {
    handle!(h).file.as_ptr()
}

/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_file_len(h: *mut Handle) -> usize {
    handle!(h).file.len()
}

/// The population size
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_pop_len(h: *mut Handle) -> usize {
    handle!(h).sim.pheno().len()
}

/// The last generation's phenotypes (0), genotypes (1) or fitnesses (2)
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_pop_ptr(h: *mut Handle, which: i32) -> *const f64 {
    let s = &handle!(h).sim;
    match which {
        1 => s.geno().as_ptr(),
        2 => s.fit().as_ptr(),
        _ => s.pheno().as_ptr(),
    }
}

/// Current values: 0 optimum θ, 1 width ω, 2 V_E, 3 mutational variance m·V_E,
/// 4 V_P, 5 h², 6 the analytical width, 7 blocks in use
///
/// # Safety
/// `h` must be a live handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn qt_param(h: *mut Handle, which: i32) -> f64 {
    let s = &handle!(h).sim;
    let o = &s.o;
    match which {
        0 => o.fitness_mean,
        1 => o.fitness_sd,
        2 => o.env_var,
        3 => o.mute_var,
        4 => o.phenotype_var,
        5 => o.heritability,
        6 => crate::options::analytical_fit_sd(o),
        7 => s.blocks() as f64,
        _ => f64::NAN,
    }
}
