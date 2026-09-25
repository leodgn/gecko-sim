//! The `eframe`/`egui` board UI, laid out like the `cs200` VS Code
//! extension: 7-segment displays, the LED matrix, the two side buttons, dip
//! switches, `b2`/`b1`/`b0` and the joystick. Buttons are clicked with the
//! mouse.
//!
//! The UI never touches the `Bus`. It draws the `BoardState` snapshot
//! published by the CPU thread, and records presses in a shared bitmask
//! that the CPU thread applies before its next batch.
//!
//! The dip switches are visual only: the hardware exposes no register for
//! them.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

// Bit of each button in the `BUTTONS` register (see
// `ressources/hardware-spec.md`). Bits 8 and 9 are unnamed in the course
// template; they are mapped to the two side buttons.
const JC: u8 = 0;
const JR: u8 = 1;
const JL: u8 = 2;
const JB: u8 = 3;
const JT: u8 = 4;
const BUTTON_1: u8 = 5;
const BUTTON_0: u8 = 6;
const BUTTON_2: u8 = 7;
const BUTTON_UNNAMED_A: u8 = 8;
const BUTTON_UNNAMED_B: u8 = 9;

/// Snapshot of the board outputs the UI draws, published by the CPU thread
/// after each batch.
pub struct BoardState {
    /// Red LED grid, indexed `[row][col]`.
    pub leds_red: [[bool; 12]; 10],
    /// Green LED grid, indexed `[row][col]`.
    pub leds_green: [[bool; 12]; 10],
    /// Blue LED grid, indexed `[row][col]`.
    pub leds_blue: [[bool; 12]; 10],
    /// Raw `SEVEN_SEGS` register value.
    pub seven_segs: u32,
}

impl BoardState {
    /// Creates a state with every LED and segment off.
    pub fn new() -> Self {
        Self {
            leds_red: [[false; 12]; 10],
            leds_green: [[false; 12]; 10],
            leds_blue: [[false; 12]; 10],
            seven_segs: 0,
        }
    }
}

/// The `eframe` application.
pub struct GeckoApp {
    board_state: Arc<Mutex<BoardState>>,
    pending_presses: Arc<AtomicU32>,
    dip_switches: [bool; 8],
}

impl GeckoApp {
    /// Creates the app. `board_state` is read on every frame;
    /// `pending_presses` gets one bit set per clicked button (bit numbers
    /// as in the `BUTTONS` register).
    pub fn new(board_state: Arc<Mutex<BoardState>>, pending_presses: Arc<AtomicU32>) -> Self {
        Self {
            board_state,
            pending_presses,
            dip_switches: [false; 8],
        }
    }

    /// Draws the 12x10 LED matrix. Each LED's color mixes its red, green
    /// and blue states.
    fn draw_leds(&self, ui: &mut eframe::egui::Ui) {
        use eframe::egui::{Color32, Rect, Sense, Stroke, pos2, vec2};

        let cell_w = 26.0;
        let cell_h = 20.0;
        let gap = 2.0;
        let size = vec2(12.0 * cell_w, 10.0 * cell_h);

        let (response, painter) = ui.allocate_painter(size, Sense::hover());
        let origin = response.rect.min;

        let (red, green, blue) = {
            let state = self.board_state.lock().unwrap();
            (state.leds_red, state.leds_green, state.leds_blue)
        };

        for row in 0..10 {
            for col in 0..12 {
                let color = Color32::from_rgb(
                    if red[row][col] { 255 } else { 0 },
                    if green[row][col] { 255 } else { 0 },
                    if blue[row][col] { 255 } else { 0 },
                );
                let top_left = origin + vec2(col as f32 * cell_w, row as f32 * cell_h);
                let rect = Rect::from_min_size(
                    pos2(top_left.x, top_left.y),
                    vec2(cell_w - gap, cell_h - gap),
                );
                painter.rect(
                    rect,
                    2.0,
                    color,
                    Stroke::new(1.0, Color32::GRAY),
                    eframe::egui::StrokeKind::Inside,
                );
            }
        }
    }

    /// Draws the four 7-segment digits, leftmost = byte 3. In each byte,
    /// bit 0 = a (top), 1 = b (top-right), 2 = c (bottom-right),
    /// 3 = d (bottom), 4 = e (bottom-left), 5 = f (top-left),
    /// 6 = g (middle). Bit 7 is ignored.
    fn draw_seven_segs(&self, ui: &mut eframe::egui::Ui) {
        use eframe::egui::{Color32, Sense, Stroke, pos2, vec2};

        let value = self.board_state.lock().unwrap().seven_segs;
        let digits = [
            (value >> 24) & 0xFF,
            (value >> 16) & 0xFF,
            (value >> 8) & 0xFF,
            value & 0xFF,
        ];

        let digit_w = 30.0;
        let digit_h = 50.0;
        let gap = 12.0;
        let thickness = 5.0;
        let total_w = 4.0 * digit_w + 3.0 * gap;

        let (response, painter) = ui.allocate_painter(vec2(total_w, digit_h), Sense::hover());
        let origin = response.rect.min;

        let lit = Color32::from_rgb(255, 40, 40);
        let unlit = Color32::from_gray(45);

        for (i, &pattern) in digits.iter().enumerate() {
            let x = origin.x + i as f32 * (digit_w + gap);
            let y = origin.y;
            let mid_y = y + digit_h / 2.0;
            let segment_color = |bit: u32| if pattern & (1 << bit) != 0 { lit } else { unlit };

            let segments = [
                (pos2(x, y), pos2(x + digit_w, y), 0), // a: top
                (pos2(x + digit_w, y), pos2(x + digit_w, mid_y), 1), // b: top-right
                (pos2(x + digit_w, mid_y), pos2(x + digit_w, y + digit_h), 2), // c: bottom-right
                (pos2(x, y + digit_h), pos2(x + digit_w, y + digit_h), 3), // d: bottom
                (pos2(x, mid_y), pos2(x, y + digit_h), 4), // e: bottom-left
                (pos2(x, y), pos2(x, mid_y), 5),       // f: top-left
                (pos2(x, mid_y), pos2(x + digit_w, mid_y), 6), // g: middle
            ];

            for (from, to, bit) in segments {
                painter.line_segment([from, to], Stroke::new(thickness, segment_color(bit)));
            }
        }
    }

    /// Records a press of the button mapped to `bit`.
    fn press(&self, bit: u8) {
        self.pending_presses.fetch_or(1 << bit, Ordering::AcqRel);
    }

    /// Draws the joystick: up, left/center/right, down.
    fn draw_joystick(&self, ui: &mut eframe::egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                if ui.button(" ^ ").clicked() {
                    self.press(JT);
                }
            });
            ui.horizontal(|ui| {
                if ui.button(" < ").clicked() {
                    self.press(JL);
                }
                if ui.button(" o ").clicked() {
                    self.press(JC);
                }
                if ui.button(" > ").clicked() {
                    self.press(JR);
                }
            });
            ui.horizontal(|ui| {
                ui.add_space(30.0);
                if ui.button(" v ").clicked() {
                    self.press(JB);
                }
            });
        });
    }

    /// Draws the two unnamed buttons (bits 8 and 9), stacked to the right of
    /// the LED matrix as in `GameOfLife.pdf`, Figure 7.
    fn draw_side_buttons(&self, ui: &mut eframe::egui::Ui) {
        ui.vertical(|ui| {
            if ui.button("B3").clicked() {
                self.press(BUTTON_UNNAMED_A);
            }
            if ui.button("B4").clicked() {
                self.press(BUTTON_UNNAMED_B);
            }
        });
    }

    /// Draws `b2`, `b1`, `b0`, left to right as in `GameOfLife.pdf`,
    /// Figure 7 (not in bit order).
    fn draw_bottom_buttons(&self, ui: &mut eframe::egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("b2").clicked() {
                self.press(BUTTON_2);
            }
            if ui.button("b1").clicked() {
                self.press(BUTTON_1);
            }
            if ui.button("b0").clicked() {
                self.press(BUTTON_0);
            }
        });
    }

    /// Draws the 8 dip switches.
    fn draw_dip_switches(&mut self, ui: &mut eframe::egui::Ui) {
        ui.horizontal(|ui| {
            for switch in &mut self.dip_switches {
                ui.checkbox(switch, "");
            }
        });
    }
}

impl eframe::App for GeckoApp {
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        // The board changes without user input, so keep repainting (~60 fps).
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));

        // Horizontal margins to center the board in the window.
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            ui.vertical(|ui| {
                ui.heading("gecko-sim");
                self.draw_seven_segs(ui);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    self.draw_leds(ui);
                    ui.add_space(10.0);
                    self.draw_side_buttons(ui);
                });
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    self.draw_dip_switches(ui);
                    ui.add_space(10.0);
                    self.draw_bottom_buttons(ui);
                    ui.add_space(10.0);
                    self.draw_joystick(ui);
                });
            });
            ui.add_space(20.0);
        });
    }
}
