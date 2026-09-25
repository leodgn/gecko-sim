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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = args().nth(1).expect("usage: gecko-sim <path.bin>");
    let file = read(path)?;

    let mut bus = Bus::new();
    bus.store_byte(MAIN_BASE, &file)
        .expect("failed to load binary");

    // `Bus` (RAM included) is NOT shared with the UI thread. Sharing the
    // whole thing behind one Mutex — simple at first — turned out to cause
    // a real, reproducible problem: a CPU thread running flat-out and a UI
    // thread both wanting the same lock settled into a stable ~90%/~70%
    // CPU standoff, confirmed with a headless repro and with real clicks
    // (via ydotool) on the actual running window, not just guessed at.
    //
    // Instead, only the tiny slice of state the UI actually needs to draw
    // (`BoardState`: LEDs + 7-seg) is shared, published by the CPU thread
    // once per batch — the lock is only ever held for a few cheap array
    // copies. Button presses go the other way through a lock-free
    // `AtomicU32` bitmask instead of a lock at all.
    let board_state = Arc::new(Mutex::new(BoardState::new()));
    let pending_presses = Arc::new(AtomicU32::new(0));

    const BATCH_DURATION: Duration = Duration::from_millis(3);
    const INSTRUCTIONS_BETWEEN_CLOCK_CHECKS: u32 = 1_000;

    let cpu_board_state = Arc::clone(&board_state);
    let cpu_pending_presses = Arc::clone(&pending_presses);
    let _handle = thread::spawn(move || {
        let mut rf = RegisterFile::new();
        let mut pc: u32 = MAIN_BASE;
        loop {
            // Drain whatever buttons were pressed since the last batch.
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

    // A fixed, non-resizable size makes most tiling window managers
    // (Hyprland included) auto-float the window instead of tiling it —
    // it doesn't make sense to tile a window that can't be resized.
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

    /// This is the integration test for step 4: a hand-encoded program
    /// (no real gol.s .bin yet), loaded through `Bus::load_bytes`, then run
    /// through the exact same `exec_one` loop `main()` will use — just
    /// bounded to 3 steps here instead of "forever", since there's no
    /// natural way to know a hand-written program is "done".
    #[test]
    fn loads_and_runs_a_tiny_hand_encoded_program() {
        // addi x1, x0, 5
        // addi x2, x0, 7
        // add  x3, x1, x2      -> x3 should end up at 12
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
