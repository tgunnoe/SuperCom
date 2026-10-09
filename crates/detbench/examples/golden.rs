//! Print the golden constants that `tests/golden.rs` compares against.
//!
//! Run on the reference platform with
//! `cargo run -p supercom-detbench --example golden`, paste the output over
//! the constants at the top of the test file, and record the date, platform,
//! and compiler in that file's header.

use supercom_detbench::command::{schedule, synthetic_commands};
use supercom_detbench::fixedpoint::mac_steps;
use supercom_detbench::floats::{basic_ops_bits, libm_exp_bits, libm_sin_bits, libm_sqrt_bits};
use supercom_detbench::hash::{chain_all, hex, scheduled_run, synthetic_states};

/// `v` as `0x` and sixteen hex digits in groups of four, as clippy wants.
fn word(v: u64) -> String {
    let h = format!("{v:016x}");
    format!("0x{}_{}_{}_{}", &h[0..4], &h[4..8], &h[8..12], &h[12..16])
}

/// `v` in decimal with a separator every three digits.
fn signed(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut grouped = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            grouped.push('_');
        }
        grouped.push(ch);
    }
    if v < 0 {
        format!("-{grouped}")
    } else {
        grouped
    }
}

fn words(name: &str, values: &[u64]) {
    println!("const {name}: [u64; {}] = [", values.len());
    for v in values {
        println!("    {},", word(*v));
    }
    println!("];");
}

fn main() {
    words("SQRT_GOLDEN", &libm_sqrt_bits());
    words("SIN_GOLDEN", &libm_sin_bits());
    words("EXP_GOLDEN", &libm_exp_bits());

    let basic = basic_ops_bits();
    println!("const BASIC_GOLDEN: [[u64; 5]; {}] = [", basic.len());
    for row in &basic {
        let cells: Vec<String> = row.iter().map(|v| word(*v)).collect();
        println!("    [{}],", cells.join(", "));
    }
    println!("];");

    let mac = mac_steps();
    let cells: Vec<String> = mac.iter().map(|v| signed(*v)).collect();
    println!(
        "const MAC_GOLDEN: [i64; {}] = [{}];",
        mac.len(),
        cells.join(", ")
    );

    let hashes = chain_all(&synthetic_states(16));
    if let (Some(first), Some(last)) = (hashes.first(), hashes.last()) {
        println!("const CHAIN_FIRST_HEX: &str = \"{}\";", hex(first));
        println!("const CHAIN_LAST_HEX: &str = \"{}\";", hex(last));
    }

    if let Ok(sched) = schedule(&synthetic_commands(), 0) {
        if let Some(last) = scheduled_run(&sched, 8).last() {
            println!("const SCHEDULED_LAST_HEX: &str = \"{}\";", hex(last));
        }
    }
}
