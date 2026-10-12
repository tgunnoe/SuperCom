//! Golden-vector and property tests for `docs/specs/determinism.md`.
//!
//! Every golden constant below was produced on x86_64-linux by
//! `cargo run -p supercom-detbench --example golden`, which calls the same
//! library functions the tests call, and was recorded on 2026-10-08 with
//! rustc 1.99.0, `libm` 0.2.16 and `fixed` 1.30.0 as `Cargo.lock` locks them; a
//! change of the `libm` lock entry is a re-golden event. No test here claims a
//! result from another architecture. The `libm` vectors hold with that crate's
//! `arch` feature off (`cargo nextest run -p supercom-detbench`) and on
//! (`cargo nextest run --workspace`, where Cargo unifies it in through the
//! plugin host's dependencies).

use std::hint::black_box;

use supercom_detbench::fixedpoint::{mac_model, mac_steps};
use supercom_detbench::floats::{
    CANONICAL_NAN_BITS, EXP_INPUTS, SIN_INPUTS, basic_ops_bits, canonical_bits, libm_exp_bits,
    libm_sin_bits, libm_sqrt_bits, std_sqrt_bits,
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

/// Each live `libm` result lies within four units in the last place of the
/// platform's own result, and the exact cases are exact. This compares `libm`
/// with the platform's own library, not with the committed constants, so a
/// failure here indicts the platform's `libm` or a change of the `libm` crate,
/// not the determinism contract.
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
