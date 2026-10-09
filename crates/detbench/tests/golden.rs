//! Golden-vector and property tests for `docs/specs/determinism.md`.
//!
//! Every golden constant below was produced on x86_64-linux by
//! `cargo run -p supercom-detbench --example golden`, which calls the same
//! library functions the tests call, and was recorded on 2026-10-08 with
//! rustc 1.99.0. No test here claims a result from another architecture. The
//! `libm` vectors hold with that crate's `arch` feature off
//! (`cargo nextest run -p supercom-detbench`) and on (`cargo nextest run
//! --workspace`, where Cargo unifies it in through the plugin host's
//! dependencies).

use std::hint::black_box;

use sha2::{Digest as _, Sha256};
use supercom_detbench::command::{Command, Schedule, ScheduleError, schedule, synthetic_commands};
use supercom_detbench::fixedpoint::{mac_model, mac_steps};
use supercom_detbench::floats::{
    CANONICAL_NAN_BITS, EXP_INPUTS, SIN_INPUTS, basic_ops_bits, canonical_bits, libm_exp_bits,
    libm_sin_bits, libm_sqrt_bits, std_sqrt_bits,
};
use supercom_detbench::hash::{
    GENESIS, chain, chain_all, first_divergence, hex, scheduled_run, synthetic_states,
};

const SQRT_GOLDEN: [u64; 12] = [
    0x0000_0000_0000_0000,
    0x3ff0_0000_0000_0000,
    0x3ff6_a09e_667f_3bcd,
    0x3ffb_b67a_e858_4caa,
    0x3fe6_a09e_667f_3bcd,
    0x3fd4_3d13_6248_490f,
    0x4009_4c58_3ada_5b53,
    0x4075_f5d3_b169_48f6,
    0x5f13_8d35_2e50_96af,
    0x20ca_2fe7_6a3f_9475,
    0x2000_0000_0000_0000,
    0x1e60_0000_0000_0000,
];
const SIN_GOLDEN: [u64; 14] = [
    0x0000_0000_0000_0000,
    0x3e45_798e_e230_8c3a,
    0x3fb9_8eae_cb8b_cb2c,
    0x3fde_aee8_744b_05f0,
    0x3fea_ed54_8f09_0cee,
    0x3fed_18f6_ead1_b446,
    0x3fc2_1038_6db6_d55b,
    0x3ff0_0000_0000_0000,
    0x3ca1_a626_3314_5c07,
    0xbfe1_689e_f5f3_4f52,
    0xbfe0_3425_b78c_4db8,
    0xbfe4_9d6e_6946_19b8,
    0x3fa2_4daa_9c52_7e96,
    0xbfdf_334c_7896_a4e3,
];
const EXP_GOLDEN: [u64; 14] = [
    0x3ff0_0000_0000_0000,
    0x3ff0_0000_0006_df38,
    0x3ff1_aec7_b35a_00d4,
    0x3ffa_6129_8e1e_069c,
    0x4005_bf0a_8b14_576a,
    0x3fd7_8b56_362c_ef38,
    0x4028_5d6f_d931_e0bb,
    0x40d5_829d_cf95_0560,
    0x3f07_cd79_b564_7c9a,
    0x48f3_494a_9b17_1bf5,
    0x7f0d_945d_f4f8_ec8e,
    0x00d1_4f2b_0fb9_307f,
    0x0000_0000_0000_0055,
    0x7fe8_1e9b_4b52_d0c9,
];
const BASIC_GOLDEN: [[u64; 5]; 6] = [
    [
        0x4010_0000_0000_0000,
        0xc000_0000_0000_0000,
        0x4008_0000_0000_0000,
        0x3fd5_5555_5555_5555,
        0x3ff0_0000_0000_0000,
    ],
    [
        0x3fd3_3333_3333_3334,
        0xbfb9_9999_9999_999a,
        0x3f94_7ae1_47ae_147c,
        0x3fe0_0000_0000_0000,
        0x3fd4_3d13_6248_490f,
    ],
    [
        0x4013_0000_0000_0000,
        0xc023_8000_0000_0000,
        0xc032_2000_0000_0000,
        0xbfd6_11a7_b961_1a7c,
        0x3ff9_4c58_3ada_5b53,
    ],
    [
        0x7fe1_ccf3_85eb_c8a0,
        0x7fe1_ccf3_85eb_c8a0,
        0x7ff0_0000_0000_0000,
        0x7fac_7b1f_3cac_7433,
        0x5fe7_dddf_6b09_5ff1,
    ],
    [
        0x4000_0000_0000_0000,
        0xc000_0000_0000_0000,
        0x0000_0000_0000_0002,
        0x0000_0000_0000_0000,
        0x1e60_0000_0000_0000,
    ],
    [
        0x40fe_240c_9ba5_e354,
        0x40fe_240c_a3d7_0a3e,
        0xc05e_dd3c_07ee_0b0b,
        0xc19d_6f34_5400_0000,
        0x4075_f5d3_b169_48f6,
    ],
];
const MAC_GOLDEN: [i64; 8] = [
    12_884_901_888,
    -30_601_641_984,
    -26_306_674_689,
    -26_306_674_690,
    -2_174_058_758_146,
    -2_178_085_289_986,
    -2_448_668_229_634,
    -2_443_365_797_971,
];
const CHAIN_FIRST_HEX: &str = "e91fef5dfc74dfe92ac0921763312b19e05a97d9b87d33772c7f4826b333e561";
const CHAIN_LAST_HEX: &str = "644064e95f3f64fab2ee5d9d99802a63e308ffb84de4a8e2af1187d9f37f56db";
const SCHEDULED_LAST_HEX: &str = "7478aa98e61302e96ed4b353805c2c38eea2a7b5e0381ab61c93aeafc0cd8247";

/// All orderings of `items` (Heap's algorithm).
fn permutations<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
    fn heap<T: Clone>(k: usize, a: &mut Vec<T>, out: &mut Vec<Vec<T>>) {
        if k <= 1 {
            out.push(a.clone());
            return;
        }
        heap(k - 1, a, out);
        for i in 0..k - 1 {
            if k % 2 == 0 {
                a.swap(i, k - 1);
            } else {
                a.swap(0, k - 1);
            }
            heap(k - 1, a, out);
        }
    }
    let mut work = items.to_vec();
    let mut out = Vec::new();
    heap(work.len(), &mut work, &mut out);
    out
}

fn cmd(tick: u64, source: u8, seq: u32, payload: &[u8]) -> Command {
    Command {
        tick,
        source,
        seq,
        payload: payload.to_vec(),
    }
}

/// The schedule as `(tick, [(source, seq)])`, for comparison with a literal.
fn keys(s: &Schedule) -> Vec<(u64, Vec<(u8, u32)>)> {
    s.iter()
        .map(|(t, cs)| (*t, cs.iter().map(|c| (c.source, c.seq)).collect()))
        .collect()
}

/// Commands run in ascending `(source, seq)` order within their tick, whatever
/// order they arrived in: all 720 arrival orders of six commands give the
/// schedule below.
#[test]
fn detbench_schedule_ignores_arrival_order() {
    let want: Vec<(u64, Vec<(u8, u32)>)> = vec![
        (0, vec![(3, 0)]),
        (2, vec![(0, 0), (1, 0), (1, 1)]),
        (5, vec![(0, 1), (2, 0)]),
    ];
    let orders = permutations(&synthetic_commands());
    assert_eq!(orders.len(), 720);
    for order in &orders {
        let got = schedule(order, 0).expect("a valid set schedules");
        assert_eq!(keys(&got), want);
    }
}

/// A command lands in the tick it is stamped for, exactly once; one stamped
/// for a tick that has started is refused, as is a repeated `(source, seq)`;
/// the refusal does not depend on arrival order.
#[test]
fn detbench_schedule_applies_at_stamped_tick() {
    let all = synthetic_commands();
    let sched = schedule(&all, 0).expect("a valid set schedules");
    let placed: usize = sched.values().map(Vec::len).sum();
    assert_eq!(placed, all.len());
    for (tick, cmds) in &sched {
        assert!(cmds.iter().all(|c| c.tick == *tick));
    }

    // A command stamped for the next tick is on time; one before it is late.
    assert!(schedule(&all, 0).is_ok());
    assert_eq!(
        schedule(&all, 3),
        Err(ScheduleError::Late {
            tick: 0,
            next_tick: 3
        })
    );
    let from_two: Vec<Command> = all.iter().filter(|c| c.tick >= 2).cloned().collect();
    assert!(schedule(&from_two, 2).is_ok());
    assert_eq!(
        schedule(&from_two, 3),
        Err(ScheduleError::Late {
            tick: 2,
            next_tick: 3
        })
    );

    // Source 1 reuses sequence number 0 at a different tick.
    let mut dup = synthetic_commands();
    dup.push(cmd(4, 1, 0, &[0xFF]));
    for order in permutations(&dup[..4]) {
        let mut set = order;
        set.push(dup[4].clone());
        set.push(dup[5].clone());
        set.push(dup[6].clone());
        assert_eq!(
            schedule(&set, 0),
            Err(ScheduleError::Duplicate { source: 1, seq: 0 })
        );
    }
}

/// The hash chain over a 16-tick synthetic run has the committed first and
/// last values, and a link is `SHA-256(prev || tick || length || state)`.
#[test]
fn detbench_tick_hash_chain_bits() {
    let hashes = chain_all(&synthetic_states(16));
    assert_eq!(hashes.len(), 16);
    assert_eq!(hex(&hashes[0]), CHAIN_FIRST_HEX);
    assert_eq!(hex(&hashes[15]), CHAIN_LAST_HEX);

    // The layout, rebuilt by hand with a different code path.
    let mut manual = [0u8; 48];
    manual[..32].copy_from_slice(&GENESIS);
    assert_eq!(
        chain(&GENESIS, 0, &[]),
        <[u8; 32]>::from(Sha256::digest(manual))
    );
    let prev = hashes[0];
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&prev);
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.push(0xFF);
    assert_eq!(
        chain(&prev, 1, &[0xFF]),
        <[u8; 32]>::from(Sha256::digest(&bytes))
    );
}

/// The primitive under the chain is SHA-256: the FIPS 180-4 "abc" vector.
#[test]
fn detbench_sha256_known_answer() {
    let got = Sha256::digest(b"abc");
    let want = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    let digest: [u8; 32] = got.into();
    assert_eq!(hex(&digest), want);
}

/// Two runs that differ first at tick `d` have equal hashes before `d` and
/// unequal hashes from `d` on, so the first unequal hash names the tick.
#[test]
fn detbench_tick_hash_localizes_divergence() {
    let base = synthetic_states(40);
    let base_hashes = chain_all(&base);
    assert_eq!(first_divergence(&base_hashes, &chain_all(&base)), None);
    for d in 0..base.len() {
        let mut bent = base.clone();
        bent[d][0] ^= 1;
        let bent_hashes = chain_all(&bent);
        assert_eq!(first_divergence(&base_hashes, &bent_hashes), Some(d));
        assert_eq!(base_hashes[..d], bent_hashes[..d]);
        for t in d..base.len() {
            assert_ne!(
                base_hashes[t], bent_hashes[t],
                "tick {t} after a change at {d}"
            );
        }
    }
    assert_eq!(first_divergence(&base_hashes[..10], &base_hashes), Some(10));
}

/// Following from the ordering and the chain: a toy run over the schedule has
/// one hash sequence for every arrival order of the same commands, with the
/// committed last value. Applying commands in arrival order instead gives
/// twelve distinct sequences, so the check can tell the two apart.
#[test]
fn detbench_scheduled_run_ignores_arrival_order() {
    let orders = permutations(&synthetic_commands());
    let mut reference: Option<Vec<[u8; 32]>> = None;
    let mut arrival_finals = std::collections::BTreeSet::new();
    for order in &orders {
        let sched = schedule(order, 0).expect("a valid set schedules");
        let run = scheduled_run(&sched, 8);
        match &reference {
            None => reference = Some(run),
            Some(first) => assert_eq!(&run, first),
        }

        // Control: the same commands grouped by tick, kept in arrival order.
        let mut naive = Schedule::new();
        for c in order {
            naive.entry(c.tick).or_default().push(c.clone());
        }
        arrival_finals.insert(scheduled_run(&naive, 8).last().copied());
    }
    let run = reference.expect("at least one order");
    assert_eq!(hex(run.last().expect("eight ticks")), SCHEDULED_LAST_HEX);
    // Only the order inside a tick matters: 3! orders at tick 2, 2! at tick 5.
    assert_eq!(arrival_finals.len(), 12);
}

/// IEEE basic operations (`+ - * /` and square root) give the committed bits
/// on this platform; `libm::sqrt` and `f64::sqrt` agree bit for bit; a
/// separate multiply and add is not fused while `mul_add` is.
#[test]
fn detbench_basic_ops_bits() {
    assert_eq!(basic_ops_bits(), BASIC_GOLDEN);
    assert_eq!(libm_sqrt_bits(), SQRT_GOLDEN);
    assert_eq!(std_sqrt_bits(), SQRT_GOLDEN);

    let a = black_box(1.0 + f64::from_bits(0x3E10_0000_0000_0000));
    let b = black_box(1.0 - f64::from_bits(0x3E10_0000_0000_0000));
    let c = black_box(-1.0);
    assert_eq!((a * b + c).to_bits(), 0.0f64.to_bits());
    assert_eq!(a.mul_add(b, c).to_bits(), 0xBC30_0000_0000_0000);
}

/// `libm::sin` over the fixed input set gives the committed bits.
#[test]
fn detbench_libm_sin_bits() {
    assert_eq!(libm_sin_bits(), SIN_GOLDEN);
}

/// `libm::exp` over the fixed input set gives the committed bits.
#[test]
fn detbench_libm_exp_bits() {
    assert_eq!(libm_exp_bits(), EXP_GOLDEN);
}

/// The committed constants are not garbage: each `libm` result lies within
/// four units in the last place of the platform's own result, and the exact
/// cases are exact.
#[test]
fn detbench_libm_within_ulps_of_std() {
    fn ordered(bits: u64) -> i64 {
        let signed = i64::from_ne_bytes(bits.to_ne_bytes());
        if signed < 0 {
            i64::MIN - signed
        } else {
            signed
        }
    }
    let sin = libm_sin_bits();
    for (i, x) in SIN_INPUTS.iter().enumerate() {
        let d = ordered(sin[i]).abs_diff(ordered(black_box(*x).sin().to_bits()));
        assert!(d <= 4, "sin({x}) is {d} ulps from std");
    }
    let exp = libm_exp_bits();
    for (i, x) in EXP_INPUTS.iter().enumerate() {
        let d = ordered(exp[i]).abs_diff(ordered(black_box(*x).exp().to_bits()));
        assert!(d <= 4, "exp({x}) is {d} ulps from std");
    }
    assert_eq!(sin[0], 0.0f64.to_bits());
    assert_eq!(exp[0], 1.0f64.to_bits());
}

/// The fixed-point multiply-accumulate sequence gives the committed raw
/// integers, and agrees with the 128-bit integer model, which includes the
/// round-toward-negative-infinity of a negative product below one raw unit.
#[test]
fn detbench_fixed_mac_bits() {
    assert_eq!(mac_steps(), MAC_GOLDEN);
    assert_eq!(mac_model(), MAC_GOLDEN);
}

/// Every NaN, whatever its sign and payload, enters hashed state as the
/// canonical quiet NaN; no other value changes.
#[test]
fn detbench_nan_canonical_bits() {
    let nans = [
        f64::NAN,
        -f64::NAN,
        f64::from_bits(0x7ff0_0000_0000_0001),
        f64::from_bits(0xfff8_dead_beef_0001),
        f64::from_bits(0x7fff_ffff_ffff_ffff),
        black_box(0.0) / black_box(0.0),
        black_box(f64::INFINITY) - black_box(f64::INFINITY),
        black_box(-1.0f64).sqrt(),
    ];
    for n in nans {
        assert!(n.is_nan());
        assert_eq!(canonical_bits(n), CANONICAL_NAN_BITS);
    }
    for v in [
        0.0,
        -0.0,
        1.0,
        -1.5,
        f64::INFINITY,
        f64::NEG_INFINITY,
        5e-324,
        f64::MAX,
    ] {
        assert_eq!(canonical_bits(v), v.to_bits());
    }
    assert_eq!(CANONICAL_NAN_BITS, 0x7ff8_0000_0000_0000);
}
