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
        0
    }
}
