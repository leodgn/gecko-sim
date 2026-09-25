//! `SEVEN_SEGS` peripheral (`0x60000000`): four digits packed into one
//! word, one byte each (`[7:0]` = digit 0, rightmost, up to `[31:24]` =
//! digit 3, leftmost). Each byte is a raw segment pattern, not a number.
//!
//! The register is plain read/write storage.

/// The `SEVEN_SEGS` register.
pub struct SevenSegs {
    value: u32,
}

impl SevenSegs {
    /// Creates the register with every segment off.
    pub fn new() -> Self {
        Self { value: 0 }
    }

    /// Returns the current register value.
    pub fn read(&self) -> u32 {
        self.value
    }

    /// Replaces the register value.
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
