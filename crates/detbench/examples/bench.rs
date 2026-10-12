//! Throughput micro-benchmark for the float-strategy decision.
//!
//! Run by hand, in release mode, on a quiet machine:
//! `cargo run --release -p supercom-detbench --example bench`.
//! It is not a test and not part of the gate. It compares the platform's
//! transcendentals with `libm`'s, floating-point multiply-accumulate with
//! fixed-point, and it reports where
//! the platform and `libm` disagree and what the default NaN looks like here.
//!
//! The example reads the clock, as the sim must not; it is a measuring tool,
//! and no result of it feeds a clause's pass or fail.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "benchmark arithmetic on counts well below 2^52"
)]

use std::hint::black_box;
use std::time::Instant;

use supercom_detbench::fixedpoint::Q;

const N: usize = 1 << 20;
const REPEATS: usize = 7;

/// A fixed integer generator (splitmix64), so every run sees the same inputs.
fn next(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// `n` values spread evenly over `[lo, hi)`.
fn inputs(seed: u64, n: usize, lo: f64, hi: f64) -> Vec<f64> {
    let mut s = seed;
    (0..n)
        .map(|_| lo + (hi - lo) * ((next(&mut s) >> 11) as f64 / (1u64 << 53) as f64))
        .collect()
}

/// Best of `REPEATS` wall-clock times of `f`, in nanoseconds per element.
fn ns_per_op(n: usize, mut f: impl FnMut()) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..REPEATS {
        let t = Instant::now();
        f();
        best = best.min(t.elapsed().as_nanos() as f64);
    }
    best / n as f64
}

/// Distance in units in the last place between two finite doubles.
fn ulps(a: f64, b: f64) -> u64 {
    fn ordered(x: f64) -> i64 {
        let bits = i64::from_ne_bytes(x.to_bits().to_ne_bytes());
        if bits < 0 { i64::MIN - bits } else { bits }
    }
    ordered(a).abs_diff(ordered(b))
}

fn transcendental(name: &str, xs: &[f64], std_f: fn(f64) -> f64, libm_f: fn(f64) -> f64) {
    let std_ns = ns_per_op(xs.len(), || {
        let mut acc = 0.0;
        for &x in xs {
            acc += std_f(black_box(x));
        }
        black_box(acc);
    });
    let libm_ns = ns_per_op(xs.len(), || {
        let mut acc = 0.0;
        for &x in xs {
            acc += libm_f(black_box(x));
        }
        black_box(acc);
    });
    let (mut differ, mut worst) = (0usize, 0u64);
    for &x in xs {
        let (a, b) = (std_f(x), libm_f(x));
        if a.to_bits() != b.to_bits() {
            differ += 1;
            worst = worst.max(ulps(a, b));
        }
    }
    println!(
        "{name:<5} std {std_ns:7.2} ns/op   libm {libm_ns:7.2} ns/op   libm/std {:5.2}x   bits differ on {differ} of {} inputs (worst {worst} ulp)",
        libm_ns / std_ns,
        xs.len()
    );
}

fn main() {
    println!(
        "arch={} os={} release={} inputs={N} repeats={REPEATS}",
        std::env::consts::ARCH,
        std::env::consts::OS,
        !cfg!(debug_assertions)
    );

    transcendental("sin", &inputs(1, N, -100.0, 100.0), f64::sin, libm::sin);
    transcendental("exp", &inputs(2, N, -20.0, 20.0), f64::exp, libm::exp);
    transcendental("sqrt", &inputs(3, N, 0.0, 1.0e6), f64::sqrt, libm::sqrt);

    // Multiply-accumulate: f64 against I32F32 and against raw i64 arithmetic.
    let a = inputs(4, N, -50.0, 50.0);
    let b = inputs(5, N, -50.0, 50.0);
    let qa: Vec<Q> = a.iter().map(|&x| Q::from_num(x)).collect();
    let qb: Vec<Q> = b.iter().map(|&x| Q::from_num(x)).collect();
    let ia: Vec<i64> = qa.iter().map(|q| q.to_bits()).collect();
    let ib: Vec<i64> = qb.iter().map(|q| q.to_bits()).collect();
    let f64_ns = ns_per_op(N, || {
        let mut acc = 0.0f64;
        for i in 0..N {
            acc += black_box(a[i]) * black_box(b[i]);
        }
        black_box(acc);
    });
    let q_ns = ns_per_op(N, || {
        let mut acc = Q::ZERO;
        for i in 0..N {
            acc = acc.wrapping_add(black_box(qa[i]).wrapping_mul(black_box(qb[i])));
        }
        black_box(acc);
    });
    let raw_ns = ns_per_op(N, || {
        let mut acc = 0i64;
        for i in 0..N {
            let wide = i128::from(black_box(ia[i])) * i128::from(black_box(ib[i]));
            acc = acc.wrapping_add((wide >> 32) as i64);
        }
        black_box(acc);
    });
    println!(
        "mac   f64 {f64_ns:7.2} ns/op   I32F32 {q_ns:7.2} ns/op   raw i64 {raw_ns:7.2} ns/op   I32F32/f64 {:5.2}x",
        q_ns / f64_ns
    );

    // What the machine does with an invalid operation, at run time.
    let zero = black_box(0.0f64);
    let inf = black_box(f64::INFINITY);
    println!(
        "nan   0.0/0.0 = {:#018x}   inf-inf = {:#018x}   sqrt(-1) = {:#018x}   f64::NAN = {:#018x}",
        (zero / black_box(0.0f64)).to_bits(),
        (inf - black_box(f64::INFINITY)).to_bits(),
        black_box(-1.0f64).sqrt().to_bits(),
        f64::NAN.to_bits()
    );
}
