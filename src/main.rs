//! gecko-sim: a fast emulator for the Gecko5 educational SoC (EPFL cs200).
//!
//! Usage: `gecko-sim <program.bin>`. The binary is loaded at `MAIN_BASE`
//! and runs from there as soon as the window opens.
//!
//! Two threads:
//! - the CPU thread owns the `Bus` and executes instructions in batches of
//!   `BATCH_DURATION`. Before each batch it applies pending button presses;
//!   after each batch it publishes the LEDs and 7-segment display to a
//!   shared `BoardState`.
//! - the main thread runs the `eframe` UI, which draws the latest
//!   `BoardState` and records button presses in a shared `AtomicU32`
//!   bitmask.

use crate::bus::{Bus, MAIN_BASE};
use crate::cpu::exec_one;
use crate::regfile::RegisterFile;
use crate::ui::BoardState;
use std::env::args;
use std::fs::read;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

mod bus;
mod cpu;
mod peripherals;
mod regfile;
mod ui;

/// Loads the program given on the command line, starts the CPU thread and
/// runs the UI until the window is closed.
///
/// If the CPU hits an error (invalid instruction, out-of-bounds access),
/// the error is printed and the CPU thread stops; the window stays open,
/// showing the last published state.
///
/// # Errors
///
/// Returns an error if the program file can't be read.
///
/// # Panics
///
/// Panics if no path is given, or if the program doesn't fit in main RAM.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = args().nth(1).expect("usage: gecko-sim <path.bin>");
    let file = read(path)?;

    let mut bus = Bus::new();
    bus.store_byte(MAIN_BASE, &file)
        .expect("failed to load binary");

    let board_state = Arc::new(Mutex::new(BoardState::new()));
    let pending_presses = Arc::new(AtomicU32::new(0));

    /// How long the CPU thread runs between two board-state publications.
    const BATCH_DURATION: Duration = Duration::from_millis(3);
    /// How many instructions run between two checks of the batch clock.
    const INSTRUCTIONS_BETWEEN_CLOCK_CHECKS: u32 = 1_000;

    let cpu_board_state = Arc::clone(&board_state);
    let cpu_pending_presses = Arc::clone(&pending_presses);
    let _handle = thread::spawn(move || {
        let mut rf = RegisterFile::new();
        let mut pc: u32 = MAIN_BASE;
        loop {
            // Apply the buttons pressed since the last batch.
            let pressed = cpu_pending_presses.swap(0, Ordering::AcqRel);
            for bit in 0..10u8 {
                if pressed & (1 << bit) != 0 {
                    bus.press_button(bit);
                }
            }

            let batch_start = Instant::now();
            loop {
                for _ in 0..INSTRUCTIONS_BETWEEN_CLOCK_CHECKS {
                    if let Err(e) = exec_one(&mut pc, &mut bus, &mut rf) {
                        println!("{:?}", e);
                        return;
                    }
                }
                if batch_start.elapsed() >= BATCH_DURATION {
                    break;
                }
            }

            // Publish a fresh snapshot for the UI to draw.
            let mut state = cpu_board_state.lock().unwrap();
            state.leds_red = bus.leds().red();
            state.leds_green = bus.leds().green();
            state.leds_blue = bus.leds().blue();
            state.seven_segs = bus.seven_segs();
        }
    });

    // Non-resizable, so tiling window managers float it instead of tiling it.
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([420.0, 500.0])
            .with_resizable(false),
        ..Default::default()
    };

    eframe::run_native(
        "gecko-sim",
        native_options,
        Box::new(|_cc| Ok(Box::new(crate::ui::GeckoApp::new(board_state, pending_presses)))),
    )
    .expect("failed to run the app");

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::bus::Bus;
    use crate::cpu::exec_one;
    use crate::regfile::RegisterFile;

    /// Loads a hand-encoded program at `MAIN_BASE` and runs it with
    /// `exec_one`, as `main` does.
    #[test]
    fn loads_and_runs_a_tiny_hand_encoded_program() {
        // addi x1, x0, 5
        // addi x2, x0, 7
        // add  x3, x1, x2  -> x3 = 12
        let program: [u32; 3] = [0x00500093, 0x00700113, 0x002081b3];

        let mut bus = Bus::new();
        let mut bytes = Vec::new();
        for instruction in program {
            bytes.extend_from_slice(&instruction.to_le_bytes());
        }
        bus.store_byte(0x80000000, &bytes);

        let mut rf = RegisterFile::new();
        let mut pc: u32 = 0x80000000;
        for _ in 0..program.len() {
            exec_one(&mut pc, &mut bus, &mut rf).unwrap();
        }

        assert_eq!(rf.read(3), 12);
    }
}
