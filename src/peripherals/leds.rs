//! `LEDS` peripheral (`0x50000000`): write-only, reads always return 0 (the
//! CPU must never be able to read back LED state — see
//! `ressources/hardware-spec.md`). The "always returns 0 on read" part is
//! handled where `Bus` dispatches to this peripheral (later in step 5),
//! not here — this module only owns the framebuffer and the write-command
//! decoding.
//!
//! Framebuffer representation: one `[[bool; 12]; 10]` per color (r/g/b) —
//! `color[row][col]`, plain and simple, one boolean per LED. This is
//! **not** the format the write command itself uses (that one's
//! bit-packed, see below, and that packing is unavoidable — it's the
//! hardware's actual wire format). But there's no reason our own storage
//! has to mirror that packing: a straightforward 2D array of booleans is
//! much easier to reason about and to update. If some future UI code
//! wants a bitmask instead (like the `cs200` extension's `LedArray_t`),
//! that conversion belongs there, isolated, not baked into this struct.
//!
//! ## Write-command format (see `hardware-spec.md` for the full table)
//!
//! ```text
//! bit 31..16 : value
//! bit 15..11 : unused
//! bit 10     : b (blue selected)
//! bit 9      : g (green selected)
//! bit 8      : r (red selected)
//! bit 7..4   : row  (0b1111 = all rows)
//! bit 3..0   : col  (0b1111 = all columns)
//! ```
//!
//! Semantics of `value`, depending on the row/col selection:
//! - all rows + all cols → every LED of the selected color(s) takes
//!   `value`'s bit 0 (i.e. bit 16 of the whole word).
//! - all rows + one col → the 10 LEDs of that column take `value`'s bits
//!   0..9 (bit 16 = top row/row 0, bit 25 = bottom row/row 9).
//! - one row + all cols → the 12 LEDs of that row take `value`'s bits
//!   0..11 (bit 16 = leftmost column, bit 27 = rightmost column).
//! - one row + one col → that single LED takes `value`'s bit 0 (bit 16).
//!
//! What to build:
//! - `pub struct Leds { r: [[bool; 12]; 10], g: [[bool; 12]; 10], b: [[bool; 12]; 10] }`
//! - `pub fn new() -> Self` — everything off (`false`).
//! - `pub fn write(&mut self, command: u32)` — decode the format above and
//!   set the right `bool`s in `r`/`g`/`b`. A row or column value of
//!   `0b1111` (`15`) means "all"; anything else is a specific index (rows
//!   0-9, columns 0-11). Tip: handle the 4 cases as 4 separate branches
//!   (all/all, all/one, one/all, one/one) rather than trying to unify them
//!   into one clever formula — the spec itself describes them as 4
//!   distinct cases.
//! - `pub fn red(&self) -> [[bool; 12]; 10]`, `pub fn green(&self) -> [[bool; 12]; 10]`,
//!   `pub fn blue(&self) -> [[bool; 12]; 10]` — plain getters (arrays of
//!   `bool` are `Copy`, returning them by value is fine here).

const ALL: u32 = 0b1111;

pub struct Leds {
    r: [[bool; 12]; 10],
    g: [[bool; 12]; 10],
    b: [[bool; 12]; 10],
}

impl Leds {
    pub fn new() -> Self {
        Self {
            r: [[false; 12]; 10],
            g: [[false; 12]; 10],
            b: [[false; 12]; 10],
        }
    }

    pub fn write(&mut self, command: u32) {
        let value = command >> 16;
        let select_b = (command >> 10) & 1 == 1;
        let select_g = (command >> 9) & 1 == 1;
        let select_r = (command >> 8) & 1 == 1;
        let row = (command >> 4) & 0b1111;
        let col = command & 0b1111;

        if row == ALL && col == ALL {
            let on = (value & 1) == 1;
            for r in 0..10 {
                for c in 0..12 {
                    self.set(select_r, select_g, select_b, r, c, on);
                }
            }
        } else if row == ALL {
            for r in 0..10 {
                let on = (value >> r) & 1 == 1;
                self.set(select_r, select_g, select_b, r as usize, col as usize, on);
            }
        } else if col == ALL {
            for c in 0..12 {
                let on = (value >> c) & 1 == 1;
                self.set(select_r, select_g, select_b, row as usize, c as usize, on);
            }
        } else {
            let on = (value & 1) == 1;
            self.set(select_r, select_g, select_b, row as usize, col as usize, on);
        }
    }

    fn set(&mut self, r: bool, g: bool, b: bool, row: usize, col: usize, on: bool) {
        if r {
            self.r[row][col] = on;
        }
        if g {
            self.g[row][col] = on;
        }
        if b {
            self.b[row][col] = on;
        }
    }

    pub fn red(&self) -> [[bool; 12]; 10] {
        self.r
    }

    pub fn green(&self) -> [[bool; 12]; 10] {
        self.g
    }

    pub fn blue(&self) -> [[bool; 12]; 10] {
        self.b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a write command from its fields, so the tests read closer to
    /// the spec table than a pile of magic hex numbers would.
    fn command(row: u32, col: u32, r: bool, g: bool, b: bool, value: u32) -> u32 {
        (value << 16)
            | ((b as u32) << 10)
            | ((g as u32) << 9)
            | ((r as u32) << 8)
            | (row << 4)
            | col
    }

    #[test]
    fn new_framebuffer_is_all_off() {
        let leds = Leds::new();
        assert_eq!(leds.red(), [[false; 12]; 10]);
        assert_eq!(leds.green(), [[false; 12]; 10]);
        assert_eq!(leds.blue(), [[false; 12]; 10]);
    }

    #[test]
    fn all_rows_all_cols_sets_every_led_of_the_selected_color() {
        let mut leds = Leds::new();
        leds.write(command(ALL, ALL, true, false, false, 1));

        assert_eq!(leds.red(), [[true; 12]; 10]);
        assert_eq!(leds.green(), [[false; 12]; 10]);
        assert_eq!(leds.blue(), [[false; 12]; 10]);
    }

    #[test]
    fn all_rows_one_col_sets_that_column_per_row() {
        let mut leds = Leds::new();
        // Row 0 on, row 1 off, row 2 on, rows 3-9 off, in column 3.
        leds.write(command(ALL, 3, true, false, false, 0b101));

        let red = leds.red();
        assert!(red[0][3]);
        assert!(!red[1][3]);
        assert!(red[2][3]);
        for row in 3..10 {
            assert!(!red[row][3]);
        }
        // Other columns must be untouched.
        assert!(!red[0][0]);
        assert!(!red[2][11]);
    }

    #[test]
    fn one_row_all_cols_sets_that_row_per_column() {
        let mut leds = Leds::new();
        // Columns 0, 1 and 11 on, the rest off, in row 5.
        leds.write(command(5, ALL, true, false, false, 0b1000_0000_0011));

        let red = leds.red();
        assert!(red[5][0]);
        assert!(red[5][1]);
        assert!(red[5][11]);
        for col in 2..11 {
            assert!(!red[5][col]);
        }
        // Other rows must be untouched.
        assert!(!red[0][0]);
    }

    #[test]
    fn one_row_one_col_sets_a_single_led() {
        let mut leds = Leds::new();
        leds.write(command(2, 7, true, false, false, 1));

        assert!(leds.red()[2][7]);
        assert!(!leds.red()[2][6]);
        assert!(!leds.red()[1][7]);
    }

    #[test]
    fn color_channels_are_independent() {
        let mut leds = Leds::new();
        leds.write(command(2, 7, true, true, false, 1));

        assert!(leds.red()[2][7]);
        assert!(leds.green()[2][7]);
        assert!(!leds.blue()[2][7]);
    }
}
