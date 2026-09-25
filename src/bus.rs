//! Memory bus: implements `cpu::traits::Memory` over flat RAM.
//!
//! No peripherals yet (that's step 5) — just two separate flat RAM regions,
//! matching the Gecko5 memory map (see `ressources/hardware-spec.md`):
//! - `0x80000000` and up: code/data/stack.
//! - `0x90001000..0x90001300`: game state (GSA, custom variables).
//!
//! What to build:
//! - `pub struct Bus { ... }` — two `Vec<u8>` (or similar), one per region,
//!   sized generously (e.g. 1 MiB for the main region, 0x300 bytes for the
//!   game-state region — that region's exact size is dictated by the
//!   memory map: `0x90001000` to `0x90001300`).
//! - `pub fn new() -> Self` — both regions zero-initialized.
//! - `impl cpu::traits::Memory for Bus`, implementing `fetch`,
//!   `read_word/half_word/byte`, `write_word/half_word/byte`.
//!
//! Key design point: for a given address, first figure out *which* region
//! it falls into and what the offset *within that region* is (address minus
//! that region's base address) — don't use the raw 32-bit address as a
//! direct index into a `Vec`, it would need gigabytes. Addresses outside
//! both regions are an error (`RiscvError::MemoryOutOfBoundsError`).
//!
//! Reminder: RISC-V is little-endian — the least significant byte is
//! stored at the lowest address. `u32::to_le_bytes`/`from_le_bytes` and
//! `u16::to_le_bytes`/`from_le_bytes` do the conversion for you.

use crate::cpu::traits::Memory;
use crate::peripherals::{buttons::Buttons, leds::Leds, random::Random, seven_segs::SevenSegs};
use std::cell::RefCell;

pub const MAIN_BASE: u32 = 0x80000000;
const MAIN_SIZE: usize = 0x100000;
pub const GAME_STATE_BASE: u32 = 0x90001000;
const GAME_STATE_SIZE: usize = 0x300;
const RANDOM: u32 = 0x40000000;
const LEDS: u32 = 0x50000000;
const SEVEN_SEGS: u32 = 0x60000000;
const BUTTONS: u32 = 0x70000004;

pub struct Bus {
    main: Vec<u8>,
    game_state: Vec<u8>,
    random: RefCell<Random>,
    leds: Leds,
    seven_segs: SevenSegs,
    buttons: Buttons,
}

impl Bus {
    pub fn new() -> Self {
        Self {
            main: vec![0; MAIN_SIZE],
            game_state: vec![0; GAME_STATE_SIZE],
            random: RefCell::new(Random::new()),
            leds: Leds::new(),
            seven_segs: SevenSegs::new(),
            buttons: Buttons::new(),
        }
    }

    /// Returns true if `addr` is in the main region, false if it's in the game-state region,
    /// alongside the byte offset within that region.
    fn locate(&self, addr: u32) -> Result<(bool, usize), crate::cpu::RiscvError> {
        if addr >= MAIN_BASE && addr < (MAIN_BASE + MAIN_SIZE as u32) {
            return Ok((true, (addr - MAIN_BASE) as usize));
        }
        if addr >= GAME_STATE_BASE && (addr < GAME_STATE_BASE + GAME_STATE_SIZE as u32) {
            return Ok((false, (addr - GAME_STATE_BASE) as usize));
        }
        Err(crate::cpu::RiscvError::MemoryOutOfBoundsError(addr))
    }

    /// Reads `num_bytes` consecutive little-endian bytes starting at `addr`
    /// and combines them into a `u32`. Shared by `read_half_word` (2 bytes)
    /// and `read_word` (4 bytes) — same logic, different bound.
    fn read_bytes(&self, addr: u32, num_bytes: u32) -> Result<u32, crate::cpu::RiscvError> {
        let mut value: u32 = 0;
        for i in 0..num_bytes {
            value |= self.read_byte(addr + i)? << (8 * i);
        }
        Ok(value)
    }

    /// Writes the `num_bytes` low-order little-endian bytes of `data`
    /// starting at `addr`. Shared by `write_half_word` (2 bytes) and
    /// `write_word` (4 bytes).
    fn write_bytes(
        &mut self,
        addr: u32,
        data: u32,
        num_bytes: u32,
    ) -> Result<(), crate::cpu::RiscvError> {
        for i in 0..num_bytes {
            let byte_to_write = (data >> (8 * i)) & 0xFF;
            self.write_byte(addr + i, byte_to_write)?;
        }
        Ok(())
    }

    /// Writes the given bytes into memory, one after another, starting at
    /// `addr`. The byte at `bytes[0]` goes to `addr`, `bytes[1]` to `addr + 1`,
    /// and so on.
    ///
    /// Used to load a whole program image (a `.bin` file's contents) into RAM
    /// in one call, instead of writing it word by word.
    pub fn store_byte(&mut self, addr: u32, bytes: &[u8]) -> Result<(), crate::cpu::RiscvError> {
        for (i, byte) in bytes.iter().enumerate() {
            self.write_byte(addr + i as u32, *byte as u32)?;
        }
        Ok(())
    }

    pub fn leds(&self) -> &Leds {
        &self.leds
    }

    pub fn seven_segs(&self) -> u32 {
        self.seven_segs.read()
    }

    pub fn press_button(&mut self, bit: u8) {
        self.buttons.press(bit);
    }
}

impl Memory for Bus {
    fn fetch(&self, pc: u32) -> Result<u32, crate::cpu::RiscvError> {
        self.read_word(pc)
    }

    fn read_word(&self, addr: u32) -> Result<u32, crate::cpu::RiscvError> {
        if addr == RANDOM {
            return Ok(self.random.borrow_mut().next());
        } else if addr == LEDS {
            return Ok(0);
        } else if addr == SEVEN_SEGS {
            return Ok(self.seven_segs.read());
        } else if addr == BUTTONS {
            return Ok(self.buttons.read());
        }
        self.read_bytes(addr, 4)
    }

    fn read_half_word(&self, addr: u32) -> Result<u32, crate::cpu::RiscvError> {
        self.read_bytes(addr, 2)
    }

    fn read_byte(&self, addr: u32) -> Result<u32, crate::cpu::RiscvError> {
        let (is_main, offset) = self.locate(addr)?;
        let byte = if is_main {
            self.main[offset]
        } else {
            self.game_state[offset]
        };
        Ok(byte as u32)
    }

    fn write_word(&mut self, addr: u32, data: u32) -> Result<(), crate::cpu::RiscvError> {
        if addr == RANDOM {
            return Ok(());
        } else if addr == LEDS {
            self.leds.write(data);
            return Ok(());
        } else if addr == SEVEN_SEGS {
            self.seven_segs.write(data);
            return Ok(());
        } else if addr == BUTTONS {
            self.buttons.clear();
            return Ok(());
        }
        self.write_bytes(addr, data, 4)
    }

    fn write_half_word(&mut self, addr: u32, data: u32) -> Result<(), crate::cpu::RiscvError> {
        self.write_bytes(addr, data, 2)
    }

    fn write_byte(&mut self, addr: u32, data: u32) -> Result<(), crate::cpu::RiscvError> {
        let (is_main, offset) = self.locate(addr)?;
        if is_main {
            self.main[offset] = data as u8;
        } else {
            self.game_state[offset] = data as u8;
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cpu::traits::Memory;

    const MAIN_BASE: u32 = 0x80000000;
    const GAME_STATE_BASE: u32 = 0x90001000;

    #[test]
    fn write_then_read_word_in_main_region() {
        let mut bus = Bus::new();
        bus.write_word(MAIN_BASE, 0x12345678).unwrap();
        assert_eq!(bus.read_word(MAIN_BASE).unwrap(), 0x12345678);
    }

    #[test]
    fn write_then_read_word_in_game_state_region() {
        let mut bus = Bus::new();
        bus.write_word(GAME_STATE_BASE, 42).unwrap();
        assert_eq!(bus.read_word(GAME_STATE_BASE).unwrap(), 42);
    }

    #[test]
    fn byte_and_half_word_access_are_little_endian() {
        let mut bus = Bus::new();
        bus.write_word(MAIN_BASE, 0xAABBCCDD).unwrap();

        assert_eq!(bus.read_byte(MAIN_BASE).unwrap(), 0xDD);
        assert_eq!(bus.read_byte(MAIN_BASE + 3).unwrap(), 0xAA);
        assert_eq!(bus.read_half_word(MAIN_BASE).unwrap(), 0xCCDD);
    }

    #[test]
    fn fetch_behaves_like_read_word() {
        let mut bus = Bus::new();
        bus.write_word(MAIN_BASE, 0xDEADBEEF).unwrap();
        assert_eq!(bus.fetch(MAIN_BASE).unwrap(), 0xDEADBEEF);
    }

    #[test]
    fn address_outside_both_regions_is_an_error() {
        let bus = Bus::new();
        // Nowhere near either region: not RAM, not (yet) a peripheral.
        assert!(bus.read_word(0x12345678).is_err());
    }

    const RANDOM: u32 = 0x40000000;
    const LEDS: u32 = 0x50000000;
    const SEVEN_SEGS: u32 = 0x60000000;
    const BUTTONS: u32 = 0x70000004;

    #[test]
    fn leds_writes_update_state_but_reads_always_return_zero() {
        let mut bus = Bus::new();
        // all rows, all cols, red selected, on: see peripherals::leds for
        // the command format.
        let all_red_on: u32 = (1 << 16) | (1 << 8) | (0b1111 << 4) | 0b1111;
        bus.write_word(LEDS, all_red_on).unwrap();

        assert_eq!(bus.read_word(LEDS).unwrap(), 0);
        assert_eq!(bus.leds().red(), [[true; 12]; 10]);
    }

    #[test]
    fn seven_segs_round_trips_through_the_bus() {
        let mut bus = Bus::new();
        bus.write_word(SEVEN_SEGS, 0x3F065B4F).unwrap();
        assert_eq!(bus.read_word(SEVEN_SEGS).unwrap(), 0x3F065B4F);
    }

    #[test]
    fn buttons_pressed_are_visible_and_cleared_by_any_cpu_write() {
        let mut bus = Bus::new();
        bus.press_button(2);
        assert_eq!(bus.read_word(BUTTONS).unwrap(), 1 << 2);

        bus.write_word(BUTTONS, 0).unwrap();
        assert_eq!(bus.read_word(BUTTONS).unwrap(), 0);
    }

    #[test]
    fn random_reads_advance_the_sequence() {
        let mut bus = Bus::new();
        let first = bus.read_word(RANDOM).unwrap();
        let second = bus.read_word(RANDOM).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn bus_can_be_shared_and_mutated_across_threads() {
        use std::sync::{Arc, Mutex};
        use std::thread;

        let bus = Arc::new(Mutex::new(Bus::new()));
        let other_thread_bus = Arc::clone(&bus);

        let handle = thread::spawn(move || {
            other_thread_bus.lock().unwrap().press_button(3);
        });
        handle.join().unwrap();

        assert_eq!(bus.lock().unwrap().read_word(BUTTONS).unwrap(), 1 << 3);
    }
}
