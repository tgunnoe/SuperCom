//! A fixed-point multiply-accumulate sequence and the integer model it must
//! match.

use fixed::types::I32F32;

/// The sim-side fixed-point format under test: 32 integer and 32 fractional
/// bits.
pub type Q = I32F32;

/// Left operands as raw 64-bit values (32 fractional bits): 1.5, -2.25, 1/3
/// rounded down, one least-significant bit negated, 1000.125, -0.75, 7, and
/// 0.0001 rounded to nearest.
pub const MAC_A: [i64; 8] = [
    0x1_8000_0000,
    -0x2_4000_0000,
    0x5555_5555,
    -1,
    0x3E8_2000_0000,
    -0xC000_0000,
    0x7_0000_0000,
    0x6_8DB9,
];

/// Right operands as raw 64-bit values: 2, 4.5, 3, one least-significant bit,
/// -0.5, 1.25, -9, and 12345.678 rounded to nearest.
pub const MAC_B: [i64; 8] = [
    0x2_0000_0000,
    0x4_8000_0000,
    0x3_0000_0000,
    1,
    -0x8000_0000,
    0x1_4000_0000,
    -0x9_0000_0000,
    0x3039_AD91_6873,
];

/// The accumulator's raw value after each step `acc = acc + a * b`, computed
/// with the `fixed` crate's wrapping operations.
#[must_use]
pub fn mac_steps() -> [i64; 8] {
    let mut acc = Q::ZERO;
    std::array::from_fn(|i| {
        let a = Q::from_bits(MAC_A[i]);
        let b = Q::from_bits(MAC_B[i]);
        acc = acc.wrapping_add(a.wrapping_mul(b));
        acc.to_bits()
    })
}

/// The same sequence in plain integer arithmetic: the product is formed in
/// 128 bits and shifted right by 32, which rounds toward negative infinity.
#[must_use]
pub fn mac_model() -> [i64; 8] {
    let mut acc: i64 = 0;
    std::array::from_fn(|i| {
        let wide = i128::from(MAC_A[i]) * i128::from(MAC_B[i]);
        // Truncating to 64 bits is the wrapping behaviour under test.
        #[allow(clippy::cast_possible_truncation)]
        let product = (wide >> 32) as i64;
        acc = acc.wrapping_add(product);
        acc
    })
}
