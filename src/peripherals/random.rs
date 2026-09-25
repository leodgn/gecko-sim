//! `RANDOM` peripheral (`0x40000000`): each read returns the next value of
//! a pseudo-random sequence.
//!
//! As on the reference simulator (`GameOfLife.pdf`, section 3.4.1), the
//! sequence is the same on every run: an xorshift32 generator with a fixed
//! seed.

/// An xorshift32 pseudo-random generator.
pub struct Random {
    state: u32,
}

impl Random {
    /// Creates a generator with the fixed seed. The seed must be non-zero,
    /// or xorshift32 would only ever return 0.
    pub fn new() -> Self {
        Self { state: 0x2545F491 }
    }

    /// Advances the generator and returns the new value.
    pub fn next(&mut self) -> u32 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 17;
        self.state ^= self.state << 5;
        return self.state;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_fresh_instances_produce_the_same_sequence() {
        let mut a = Random::new();
        let mut b = Random::new();

        for _ in 0..10 {
            assert_eq!(a.next(), b.next());
        }
    }

    #[test]
    fn consecutive_values_are_not_all_identical() {
        let mut rng = Random::new();
        let first = rng.next();
        let second = rng.next();
        let third = rng.next();

        assert!(first != second || second != third);
    }
}
