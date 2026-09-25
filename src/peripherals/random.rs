//! `RANDOM` peripheral (`0x40000000`): each read returns the next value of
//! a pseudo-random sequence. Per `GameOfLife.pdf` section 3.4.1, the real
//! hardware/simulator is expected to return the **same sequence on every
//! run** — not truly random, just a fixed, reproducible stream of numbers.
//!
//! What to build: a `Random` type wrapping a simple PRNG (xorshift32 is a
//! good fit: fast, a handful of lines, no external crate needed).
//!
//! - `pub fn new() -> Self` — seeded with a **fixed, hardcoded** constant
//!   (not derived from system time or anything else that changes between
//!   runs). xorshift32 requires a non-zero seed, or it would get stuck
//!   returning 0 forever — pick any non-zero `u32` constant.
//! - `pub fn next(&mut self) -> u32` — advances the internal state and
//!   returns the new value. The classic xorshift32 step:
//!   ```text
//!   state ^= state << 13
//!   state ^= state >> 17
//!   state ^= state << 5
//!   ```
//!   applied in that order, each line updating `state` before the next one
//!   reads it, then return the new `state`.

pub struct Random {
    state: u32,
}

impl Random {
    pub fn new() -> Self {
        Self { state: 0x2545F491 }
    }

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

        // A sanity check against a broken implementation that just returns
        // the seed unchanged every time.
        assert!(first != second || second != third);
    }
}
