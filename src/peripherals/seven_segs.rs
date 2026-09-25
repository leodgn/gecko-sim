//! `SEVEN_SEGS` peripheral (`0x60000000`): 4 digits, one byte each, packed
//! into a single 32-bit word (`[7:0]` = digit 0 / rightmost, ... `[31:24]`
//! = digit 3 / leftmost — see `ressources/hardware-spec.md`). Each byte is
//! a segment pattern, not a decimal digit (see `font_data` in `gol.s`).
//!
//! The PDF doesn't document read behavior for this register (unlike
//! `LEDS`, which is explicitly write-only) — so it's plain, ordinary
//! read/write storage, no special decoding needed at this layer. The
//! segment-pattern interpretation only matters once the UI draws the
//! digits (step 7).
//!
//! What to build: the simplest of the 4 peripherals — a single `u32`
//! wrapped in a struct, with a getter and a setter.

pub struct SevenSegs {
    value: u32,
}

impl SevenSegs {
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn read(&self) -> u32 {
        self.value
    }

    pub fn write(&mut self, value: u32) {
        self.value = value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_register_starts_at_zero() {
        let seven_segs = SevenSegs::new();
        assert_eq!(seven_segs.read(), 0);
    }

    #[test]
    fn write_then_read_returns_the_same_value() {
        let mut seven_segs = SevenSegs::new();
        seven_segs.write(0x3F065B4F);
        assert_eq!(seven_segs.read(), 0x3F065B4F);
    }
}
