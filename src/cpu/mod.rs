//! RV32I core, vendored from `lib-rv32` (`trmckay/lib-rv32`, MIT) with a
//! bugfix. See `docs/superpowers/specs/2026-09-24-gecko-sim-design.md` for
//! why we vendor instead of depending on the crate, and what the bug is.
//!
//! ## Recipe to fill in this module
//!
//! `lib-rv32` is cloned into `lib-rv32/` at the project root (gitignored).
//! Copy these files into `src/cpu/`, renaming as needed:
//!
//! | Source (in lib-rv32/)             | Destination            |
//! |------------------------------------|------------------------|
//! | `common/src/bits.rs`              | `src/cpu/bits.rs`      |
//! | `common/src/constants.rs`         | `src/cpu/constants.rs` |
//! | `common/src/instructions.rs`      | `src/cpu/instructions.rs` |
//! | `isa-sim/src/error.rs`            | `src/cpu/error.rs`     |
//! | `isa-sim/src/traits.rs`           | `src/cpu/traits.rs`    |
//! | `isa-sim/src/decode.rs`           | `src/cpu/decode.rs`    |
//! | `isa-sim/src/exec.rs`             | `src/cpu/exec.rs`      |
//!
//! Do **not** copy `isa-sim/src/util.rs`, `isa-sim/src/test.rs`,
//! `common/src/util.rs`, or either `lib.rs` — not needed here (`util.rs`
//! is for parsing integers in the official CLI, `test.rs` is superseded
//! by the tests further down in this file).
//!
//! **Two systematic replacements** to make in the copied files (plain
//! search/replace, nothing subtle):
//!
//! 1. `crate::` → `crate::cpu::` — in the original files, `crate::` meant
//!    "the root of the `lib-rv32-isa` crate". Once copied here, that root
//!    is called `cpu` (this module), not the root of `gecko-sim`.
//! 2. `lib_rv32_common::` → `crate::cpu::` — same reason: `common` and
//!    `isa-sim` used to be two separate crates, now everything lives
//!    inside `cpu`.
//!
//! (`bits.rs`, `constants.rs`, `instructions.rs` have no references to
//! replace, they're self-contained — copy them as-is.)
//!
//!
//! **The bug to fix** (see the spec for details): in `exec.rs`, the branch
//! that distinguishes `add` from `sub` (same `func3`, normally
//! distinguished by `func7`) mistakenly tests `decode_func3!(ir)` instead
//! of `decode_func7!(ir)`. Search for `FUNC7_SUB` in the copied file, the
//! faulty `match` is right above it. Leave a comment explaining the fix
//! once corrected.

mod bits;
mod constants;
mod decode;
mod error;
mod exec;
mod instructions;
pub mod traits;

pub use error::RiscvError;
pub use exec::exec_one;

#[cfg(test)]
mod tests {
    use super::traits::Memory;
    use crate::regfile::RegisterFile;

    /// Minimal `Memory` for these tests only — a flat byte array, real
    /// endianness handling, no address translation. The real `Bus` (with
    /// the actual Gecko5 memory map and peripherals) comes in step 3.
    struct FakeMemory {
        bytes: Vec<u8>,
    }

    impl FakeMemory {
        fn new(size: usize) -> Self {
            FakeMemory {
                bytes: vec![0; size],
            }
        }

        /// Places raw instruction words at addresses 0, 4, 8, ...
        fn with_program(words: &[u32]) -> Self {
            let mut mem = FakeMemory::new(64);
            for (i, word) in words.iter().enumerate() {
                mem.write_word((i * 4) as u32, *word).unwrap();
            }
            mem
        }
    }

    impl Memory for FakeMemory {
        fn fetch(&self, pc: u32) -> Result<u32, super::RiscvError> {
            self.read_word(pc)
        }

        fn read_word(&self, addr: u32) -> Result<u32, super::RiscvError> {
            let addr = addr as usize;
            Ok(u32::from_le_bytes([
                self.bytes[addr],
                self.bytes[addr + 1],
                self.bytes[addr + 2],
                self.bytes[addr + 3],
            ]))
        }

        fn read_half_word(&self, addr: u32) -> Result<u32, super::RiscvError> {
            let addr = addr as usize;
            Ok(u16::from_le_bytes([self.bytes[addr], self.bytes[addr + 1]]) as u32)
        }

        fn read_byte(&self, addr: u32) -> Result<u32, super::RiscvError> {
            Ok(self.bytes[addr as usize] as u32)
        }

        fn write_word(&mut self, addr: u32, data: u32) -> Result<(), super::RiscvError> {
            let addr = addr as usize;
            self.bytes[addr..addr + 4].copy_from_slice(&data.to_le_bytes());
            Ok(())
        }

        fn write_half_word(&mut self, addr: u32, data: u32) -> Result<(), super::RiscvError> {
            let addr = addr as usize;
            self.bytes[addr..addr + 2].copy_from_slice(&(data as u16).to_le_bytes());
            Ok(())
        }

        fn write_byte(&mut self, addr: u32, data: u32) -> Result<(), super::RiscvError> {
            self.bytes[addr as usize] = data as u8;
            Ok(())
        }
    }

    #[test]
    fn lui_loads_upper_immediate_and_advances_pc() {
        // lui x5, 4  ->  x5 = 4 << 12
        let mut mem = FakeMemory::with_program(&[super::instructions::LUI_X5_4]);
        let mut rf = RegisterFile::new();
        let mut pc: u32 = 0;

        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap();

        assert_eq!(rf.read(5), 4 << 12);
        assert_eq!(pc, 4);
    }

    #[test]
    fn addi_writes_destination_register() {
        // addi x6, x0, 1  ->  x6 = 1
        let mut mem = FakeMemory::with_program(&[super::instructions::ADDI_X6_X0_1]);
        let mut rf = RegisterFile::new();
        let mut pc: u32 = 0;

        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap();

        assert_eq!(rf.read(6), 1);
    }

    #[test]
    fn beq_branches_when_operands_are_equal() {
        // beq x5, x5, 12  ->  always taken (a register always equals itself)
        let mut mem = FakeMemory::with_program(&[super::instructions::BEQ_X5_X5_12]);
        let mut rf = RegisterFile::new();
        let mut pc: u32 = 0;

        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap();

        assert_eq!(pc, 12);
    }

    /// Regression test for the add/sub bug found during the lib-rv32 spike
    /// (confirmed by actually running this instruction against the
    /// unpatched library: 5 - 5 came out as 10, i.e. `sub` behaved like
    /// `add`). `gol.s` uses `sub` for its speed countdown timer, so this
    /// must hold for the real program to behave correctly.
    #[test]
    fn sub_computes_a_difference_not_a_sum() {
        // sub x5, x5, x5  ->  x5 - x5, which is 0 no matter what x5 holds
        let mut mem = FakeMemory::with_program(&[super::instructions::SUB_X5_X5_X5]);
        let mut rf = RegisterFile::new();
        rf.write(5, 99); // if this were `add`, result would be 198, not 0
        let mut pc: u32 = 0;

        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap();

        assert_eq!(rf.read(5), 0);
    }

    /// Runs a single branch instruction (branch offset: +8) with the given
    /// operands in x1/x2 (or whatever registers the encoding names) and
    /// returns whether it was taken.
    fn branch_taken(instruction: u32, regs: &[(u8, u32)]) -> bool {
        let mut mem = FakeMemory::with_program(&[instruction]);
        let mut rf = RegisterFile::new();
        for &(reg, value) in regs {
            rf.write(reg, value);
        }
        let mut pc: u32 = 0;

        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap();

        match pc {
            8 => true,
            4 => false,
            other => panic!("unexpected pc after branch: {other}"),
        }
    }

    // Hand-encoded B-type instructions, all with a +8 offset.
    const BLT_X1_X2_8: u32 = 0x0020c463;
    const BGE_X1_X2_8: u32 = 0x0020d463;
    const BLTU_X1_X2_8: u32 = 0x0020e463;
    const BGEU_X1_X2_8: u32 = 0x0020f463;
    /// `bgtz x9, 8` is a pseudo-instruction for `blt x0, x9, 8`.
    const BGTZ_X9_8: u32 = 0x00904463;

    const MINUS_ONE: u32 = -1i32 as u32;

    /// Regression test for the upstream lib-rv32 bug behind
    /// `docs/known-issues/freeze-after-speeding-up.md`: `blt`/`bge` compared
    /// their operands as unsigned, so -1 looked like 0xFFFFFFFF, i.e. huge.
    #[test]
    fn blt_and_bge_compare_as_signed() {
        assert!(
            branch_taken(BLT_X1_X2_8, &[(1, MINUS_ONE), (2, 1)]),
            "-1 < 1"
        );
        assert!(
            !branch_taken(BGE_X1_X2_8, &[(1, MINUS_ONE), (2, 1)]),
            "!(-1 >= 1)"
        );
        assert!(
            !branch_taken(BLT_X1_X2_8, &[(1, 1), (2, MINUS_ONE)]),
            "!(1 < -1)"
        );
        assert!(
            branch_taken(BGE_X1_X2_8, &[(1, 1), (2, MINUS_ONE)]),
            "1 >= -1"
        );
    }

    /// The exact instruction `wait` in `gol.s` loops on: `sub s1, s1, a3`
    /// then `bgtz s1, 1b`. With a speed that doesn't divide `MAX_WAIT_TIME`
    /// (e.g. 5), the counter goes from a small positive value straight to a
    /// negative one, and the loop must exit there. With an unsigned
    /// comparison, it instead kept going for ~2^32 / speed iterations: the
    /// "freeze" (minutes of busy-looping with nothing on screen changing).
    #[test]
    fn bgtz_is_not_taken_for_a_negative_register() {
        assert!(!branch_taken(BGTZ_X9_8, &[(9, -2i32 as u32)]));
        assert!(!branch_taken(BGTZ_X9_8, &[(9, 0)]));
        assert!(branch_taken(BGTZ_X9_8, &[(9, 3)]));
    }

    #[test]
    fn bltu_and_bgeu_compare_as_unsigned() {
        assert!(
            !branch_taken(BLTU_X1_X2_8, &[(1, MINUS_ONE), (2, 1)]),
            "!(0xFFFFFFFF <u 1)"
        );
        assert!(
            branch_taken(BGEU_X1_X2_8, &[(1, MINUS_ONE), (2, 1)]),
            "0xFFFFFFFF >=u 1"
        );
    }

    /// Upstream lib-rv32 implemented `bgeu` as a strict `>`.
    #[test]
    fn bgeu_is_taken_when_operands_are_equal() {
        assert!(branch_taken(BGEU_X1_X2_8, &[(1, 7), (2, 7)]));
    }

    #[test]
    fn store_then_load_round_trips_through_memory() {
        // sw x2, 0(x1) ; lw x3, 0(x1)  -- store x2 at [x1], then load it into x3
        let sw_x2_0_x1: u32 = 0x0020a023;
        let lw_x3_0_x1: u32 = 0x0000a183;
        let mut mem = FakeMemory::with_program(&[sw_x2_0_x1, lw_x3_0_x1]);
        let mut rf = RegisterFile::new();
        rf.write(1, 0x20); // address, chosen to not overlap the program at [0x0, 0x8)
        rf.write(2, 1234); // value to store
        let mut pc: u32 = 0;

        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap(); // sw
        super::exec_one(&mut pc, &mut mem, &mut rf).unwrap(); // lw

        assert_eq!(rf.read(3), 1234);
    }
}
