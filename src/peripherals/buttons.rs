//! `BUTTONS` peripheral (`0x70000004`): 10 physical buttons, one bit each
//! (bits 0-9), rest reserved. See `ressources/hardware-spec.md` for the
//! named bits (JC/JR/JL/JB/JT, BUTTON_0/1/2) — not needed at this layer,
//! that mapping only matters once the UI exists (step 7).
//!
//! Hardware semantics (from the spec):
//! - A bit goes to 1 on a "falling edge" (button released → pressed). It
//!   **stays** 1 until the program clears it — pressing again while it's
//!   already 1 changes nothing.
//! - **Any** write from the CPU clears the **entire** register at once
//!   (you can't clear a single bit — the hardware doesn't support that).
//! - Reads just return the current register value, no side effect.
//!
//! What to build:
//! - `pub struct Buttons { register: u32 }`
//! - `pub fn new() -> Self` — register starts at 0.
//! - `pub fn press(&mut self, bit: u8)` — simulates a UI click: sets that
//!   bit to 1 (leaves every other bit untouched).
//! - `pub fn read(&self) -> u32` — the current register value.
//! - `pub fn clear(&mut self)` — simulates a CPU write: resets the whole
//!   register to 0, regardless of what was pressed.

pub struct Buttons {
    register: u32,
}

impl Buttons {
    pub fn new() -> Self {
        Self { register: 0 }
    }

    pub fn press(&mut self, bit: u8) {
        self.register |= 1 << bit;
    }

    pub fn read(&self) -> u32 {
        self.register
    }

    pub fn clear(&mut self) {
        self.register = 0
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_register_starts_at_zero() {
        let buttons = Buttons::new();
        assert_eq!(buttons.read(), 0);
    }

    #[test]
    fn pressing_a_button_sets_its_bit() {
        let mut buttons = Buttons::new();
        buttons.press(3);
        assert_eq!(buttons.read(), 0b1000);
    }

    #[test]
    fn pressing_the_same_button_twice_changes_nothing() {
        let mut buttons = Buttons::new();
        buttons.press(3);
        buttons.press(3);
        assert_eq!(buttons.read(), 0b1000);
    }

    #[test]
    fn different_buttons_are_independent() {
        let mut buttons = Buttons::new();
        buttons.press(0);
        buttons.press(4);
        assert_eq!(buttons.read(), 0b10001);
    }

    #[test]
    fn any_clear_resets_the_whole_register() {
        let mut buttons = Buttons::new();
        buttons.press(0);
        buttons.press(4);
        buttons.clear();
        assert_eq!(buttons.read(), 0);
    }
}
