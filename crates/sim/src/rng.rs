/// The sim's seeded pseudo-random generator (DET-025); written in this crate
/// so its output is the same on every platform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    state: u64,
}

impl Rng {
    /// A generator whose whole state is derived from `seed`.
    #[must_use]
    pub fn from_seed(seed: u64) -> Rng {
        Rng { state: seed }
    }

    /// The generator's internal state, as serialized (DET-009).
    #[must_use]
    pub fn state(&self) -> u64 {
        self.state
    }

    /// The next output.
    pub fn next_u64(&mut self) -> u64 {
        // splitmix64: 64 bits of state, which the serialization writes as one
        // LE64 (DET-009); wrapping arithmetic on integers only (DET-025).
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}
