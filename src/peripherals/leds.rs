//! `LEDS` peripheral (`0x50000000`): a 10-row by 12-column matrix of RGB
//! LEDs, driven by write commands. It is write-only: reads return 0 (handled
//! by `Bus`).
//!
//! ## Write-command format
//!
//! ```text
//! bit 31..16 : value
//! bit 15..11 : unused
//! bit 10     : blue selected
//! bit 9      : green selected
//! bit 8      : red selected
//! bit 7..4   : row (0b1111 = all rows)
//! bit 3..0   : column (0b1111 = all columns)
//! ```
//!
//! Only the selected colors are updated. How `value` is applied depends on
//! the row/column selection:
//! - all rows, all columns: every LED takes `value` bit 0.
//! - all rows, one column: row `r` takes `value` bit `r` (0-9).
//! - one row, all columns: column `c` takes `value` bit `c` (0-11).
//! - one row, one column: that LED takes `value` bit 0.

/// Row/column selector value meaning "all rows" or "all columns".
const ALL: u32 = 0b1111;

/// The LED matrix state, one on/off grid per color, indexed `[row][col]`.
pub struct Leds {
    r: [[bool; 12]; 10],
    g: [[bool; 12]; 10],
    b: [[bool; 12]; 10],
}

impl Leds {
    /// Creates the matrix with every LED off.
    pub fn new() -> Self {
        Self {
            r: [[false; 12]; 10],
            g: [[false; 12]; 10],
            b: [[false; 12]; 10],
        }
    }

    /// Applies a write command (see the module docs for the format).
    ///
    /// # Panics
    ///
    /// Panics if the command selects a row in 10..=14 or a column in
    /// 12..=14, which don't exist.
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

    /// Sets the LED at `row`, `col` to `on` in each selected color.
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

    /// Returns the red grid.
    pub fn red(&self) -> [[bool; 12]; 10] {
        self.r
    }

    /// Returns the green grid.
    pub fn green(&self) -> [[bool; 12]; 10] {
        self.g
    }

    /// Returns the blue grid.
    pub fn blue(&self) -> [[bool; 12]; 10] {
        self.b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a write command from its fields.
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
