//! gecko-sim: a fast emulator for the Gecko5 educational SoC (EPFL cs200).
//!
//! Usage: `gecko-sim <program.s|program.bin>`. A `.s` file is first
//! assembled with the local RISC-V GNU toolchain (see `assembler::native`).
//! The binary is loaded at `MAIN_BASE` and runs from there as soon as the
//! window opens.
//!
//! Two threads:
//! - the CPU thread owns the `Bus` and executes instructions in batches of
//!   `BATCH_DURATION`. Before each batch it applies pending button presses;
//!   after each batch it publishes the LEDs and 7-segment display to a
//!   shared `BoardState`.
//! - the main thread runs the `eframe` UI, which draws the latest
//!   `BoardState` and records button presses in a shared `AtomicU32`
//!   bitmask.

use runner::CpuRunner;
use std::env::args;
use std::fs::{read, read_to_string};
use std::path::Path;
use std::{process, thread};

use crate::assembler::native::assemble;

mod assembler;
mod bus;
mod cpu;
mod peripherals;
mod regfile;
mod runner;
mod ui;

/// Loads the program given on the command line (assembling it first if
/// it's a `.s`), starts the CPU thread and runs the UI until the window is
/// closed.
///
/// If the file is neither a `.s` nor a `.bin`, or if assembling it fails,
/// the reason is printed on stderr (the assembler's own messages,
/// verbatim) and the process exits with status 1 before any window opens.
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
    let path = args()
        .nth(1)
        .expect("usage: gecko-sim <program.s|program.bin>");
    let program = match Path::new(&path).extension().and_then(|ext| ext.to_str()) {
        Some("s") => {
            let source = read_to_string(&path)?;
            match assemble(&path, &source) {
                Ok(bin) => bin,
                Err(e) => {
                    eprintln!("{e}");
                    process::exit(1);
                }
            }
        }
        Some("bin") => read(&path)?,
        _ => {
            eprintln!("unsupported file: {path} (expected a .s or .bin file)");
            process::exit(1);
        }
    };

    let mut runner = CpuRunner::new(&program).expect("failed to load program");
    let board_state = runner.board_state();
    let pending_presses = runner.pending_presses();
    thread::spawn(move || {
        loop {
            if let Err(e) = runner.run_batch() {
                println!("{:?}", e);
                return;
            }
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
        Box::new(|_cc| {
            Ok(Box::new(crate::ui::GeckoApp::new(
                board_state,
                pending_presses,
            )))
        }),
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
