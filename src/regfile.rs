//! The RV32I integer register file.

/// The 32 general-purpose registers `x0`..`x31`.
///
/// `x0` is hardwired to zero: writes to it are ignored.
pub struct RegisterFile {
    registers: [u32; 32],
}

impl RegisterFile {
    /// Creates a register file with every register set to 0.
    pub fn new() -> Self {
        Self {
            registers: [0u32; 32],
        }
    }

    /// Returns the value of register `x{num}`.
    ///
    /// # Panics
    ///
    /// Panics if `num >= 32`.
    pub fn read(&self, num: u8) -> u32 {
        self.registers[num as usize]
    }

    /// Sets register `x{num}` to `data`. Writes to `x0` are ignored.
    ///
    /// # Panics
    ///
    /// Panics if `num >= 32`.
    pub fn write(&mut self, num: u8, data: u32) {
        if num == 0 {
        } else {
            self.registers[num as usize] = data
        }
    }
}

/// Adapter for the CPU core. Never fails: register numbers come from 5-bit
/// instruction fields, so they are always in range.
impl crate::cpu::traits::RegisterFile for RegisterFile {
    fn read(&self, num: u8) -> Result<u32, crate::cpu::RiscvError> {
        Ok(RegisterFile::read(self, num))
    }

    fn write(&mut self, num: u8, data: u32) -> Result<(), crate::cpu::RiscvError> {
        RegisterFile::write(self, num, data);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_registers_start_at_zero() {
        let rf = RegisterFile::new();
        for i in 0..32u8 {
            assert_eq!(rf.read(i), 0, "x{i} should be 0 initially");
        }
    }

    #[test]
    fn write_then_read_a_normal_register() {
        let mut rf = RegisterFile::new();
        rf.write(5, 42);
        assert_eq!(rf.read(5), 42);
    }

    #[test]
    fn x0_always_stays_zero() {
        let mut rf = RegisterFile::new();
        rf.write(0, 0xDEADBEEF);
        assert_eq!(
            rf.read(0),
            0,
            "x0 must stay hardwired to zero even after a write"
        );
    }

    #[test]
    fn registers_are_independent() {
        let mut rf = RegisterFile::new();
        rf.write(1, 111);
        rf.write(2, 222);
        assert_eq!(rf.read(1), 111);
        assert_eq!(rf.read(2), 222);
        // Writing to x2 must not affect x1.
        rf.write(2, 999);
        assert_eq!(rf.read(1), 111);
    }
}
