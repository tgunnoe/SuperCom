//! Fixed input sets and the operations whose bit patterns are committed as
//! golden vectors: IEEE basic operations, `libm` transcendentals, and the
//! canonical NaN rule.

use std::hint::black_box;

/// Inputs for square root: zero, exact squares, a subnormal, the smallest
/// normal, and large and small ordinary values.
pub const SQRT_INPUTS: [f64; 12] = [
    0.0,
    1.0,
    2.0,
    3.0,
    0.5,
    0.1,
    10.0,
    123_456.789,
    1e300,
    1e-300,
    f64::MIN_POSITIVE,
    5e-324,
];

/// Inputs for sine: small, around the multiples of pi/2, and large arguments
/// that take the long argument-reduction path.
pub const SIN_INPUTS: [f64; 14] = [
    0.0,
    1e-8,
    0.1,
    0.5,
    1.0,
    2.0,
    3.0,
    std::f64::consts::FRAC_PI_2,
    std::f64::consts::PI,
    10.0,
    100.0,
    -0.7,
    1e5,
    1e10,
];

/// Inputs for the exponential: around zero, ordinary values, and the ends of
/// the range where the result is huge or subnormal.
pub const EXP_INPUTS: [f64; 14] = [
    0.0, 1e-10, 0.1, 0.5, 1.0, -1.0, 2.5, 10.0, -10.0, 100.0, 700.0, -700.0, -740.0, 709.5,
];

/// Operand pairs for the basic operations.
pub const BASIC_PAIRS: [(f64, f64); 6] = [
    (1.0, 3.0),
    (0.1, 0.2),
    (-2.5, 7.25),
    (1e308, 10.0),
    (5e-324, 2.0),
    (123_456.789, -0.001),
];

/// The bit pattern of the canonical quiet NaN: sign clear, only the top
/// significand bit set. It is the pattern wasmtime's NaN canonicalization
/// produces (`crates/plugin/tests/sandbox.rs`).
pub const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

/// Apply `f` to every input and return the result bit patterns.
#[must_use]
pub fn bits_of<const N: usize>(inputs: &[f64; N], f: fn(f64) -> f64) -> [u64; N] {
    std::array::from_fn(|i| f(black_box(inputs[i])).to_bits())
}

/// Bit patterns of `a + b`, `a - b`, `a * b`, `a / b`, and the square root of
/// `a.abs()`, for every pair, using the platform's own operations.
#[must_use]
pub fn basic_ops_bits() -> [[u64; 5]; 6] {
    std::array::from_fn(|i| {
        let (a, b) = (black_box(BASIC_PAIRS[i].0), black_box(BASIC_PAIRS[i].1));
        [
            (a + b).to_bits(),
            (a - b).to_bits(),
            (a * b).to_bits(),
            (a / b).to_bits(),
            a.abs().sqrt().to_bits(),
        ]
    })
}

/// `libm::sqrt` over [`SQRT_INPUTS`], as bit patterns.
#[must_use]
pub fn libm_sqrt_bits() -> [u64; 12] {
    bits_of(&SQRT_INPUTS, libm::sqrt)
}

/// `f64::sqrt` over [`SQRT_INPUTS`], as bit patterns.
#[must_use]
pub fn std_sqrt_bits() -> [u64; 12] {
    bits_of(&SQRT_INPUTS, f64::sqrt)
}

/// `libm::sin` over [`SIN_INPUTS`], as bit patterns.
#[must_use]
pub fn libm_sin_bits() -> [u64; 14] {
    bits_of(&SIN_INPUTS, libm::sin)
}

/// `libm::exp` over [`EXP_INPUTS`], as bit patterns.
#[must_use]
pub fn libm_exp_bits() -> [u64; 14] {
    bits_of(&EXP_INPUTS, libm::exp)
}

/// The bit pattern a value takes when it enters hashed state: every NaN, of
/// either sign and any payload, becomes [`CANONICAL_NAN_BITS`]; every other
/// value keeps its own bits.
#[must_use]
pub fn canonical_bits(x: f64) -> u64 {
    if x.is_nan() {
        CANONICAL_NAN_BITS
    } else {
        x.to_bits()
    }
}
