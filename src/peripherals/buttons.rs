//! `BUTTONS` peripheral (`0x70000004`): one sticky bit per physical button
//! (bits 0-9, the rest reserved).
//!
//! - A press sets the button's bit to 1. It stays 1 until the program
//!   clears it; pressing again in the meantime has no effect.
//! - Any CPU write clears the whole register, whatever the written value.
//! - Reads have no side effect.

/// The `BUTTONS` register.
pub struct Buttons {
    register: u32,
}

impl Buttons {
    /// Creates the register with no button pressed.
    pub fn new() -> Self {
        Self { register: 0 }
    }

    /// Sets the bit of the button that was pressed. Other bits are unchanged.
    pub fn press(&mut self, bit: u8) {
        self.register |= 1 << bit;
    }

    /// Returns the current register value.
    pub fn read(&self) -> u32 {
        self.register
    }

    /// Clears every bit, as a CPU write does.
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
