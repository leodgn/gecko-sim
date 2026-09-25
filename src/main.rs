use crate::bus::{Bus, MAIN_BASE};
use crate::cpu::exec_one;
use crate::regfile::RegisterFile;
use std::env::args;
use std::fs::read;
use std::sync::{Arc, Mutex};
use std::thread;

mod bus;
mod cpu;
mod peripherals;
mod regfile;
mod ui;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = args().nth(1).expect("usage: gecko-sim <path.bin>");
    let file = read(path)?;

    let bus = Arc::new(Mutex::new(Bus::new()));
    bus.lock()
        .unwrap()
        .store_byte(MAIN_BASE, &file)
        .expect("failed to load binary");

    const LOCK_HOLD_TIME: std::time::Duration = std::time::Duration::from_millis(2);
    const SLEEP_BETWEEN_BATCHES: std::time::Duration = std::time::Duration::from_millis(1);
    const INSTRUCTIONS_BETWEEN_CLOCK_CHECKS: u32 = 1_000;

    let cpu_bus = Arc::clone(&bus);
    let _handle = thread::spawn(move || {
        let mut rf = RegisterFile::new();
        let mut pc: u32 = 0x80000000;
        'outer: loop {
            let mut bus = cpu_bus.lock().unwrap();
            let batch_start = std::time::Instant::now();
            loop {
                for _ in 0..INSTRUCTIONS_BETWEEN_CLOCK_CHECKS {
                    if let Err(e) = exec_one(&mut pc, &mut *bus, &mut rf) {
                        println!("{:?}", e);
                        break 'outer;
                    }
                }
                if batch_start.elapsed() >= LOCK_HOLD_TIME {
                    break;
                }
            }
            drop(bus);
            thread::sleep(SLEEP_BETWEEN_BATCHES);
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
        Box::new(|_cc| Ok(Box::new(crate::ui::GeckoApp::new(bus)))),
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
