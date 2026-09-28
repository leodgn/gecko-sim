//! gecko-sim: a fast emulator for the Gecko5 board (RV32I CPU, LED matrix,
//! 7-segment display, buttons).
//!
//! Two builds share the emulator (`bus`, `cpu`, `peripherals`, `runner`)
//! and the UI (`ui`), and differ only in their entry point:
//! - native (`native`): the program comes from the command line, the CPU
//!   runs on its own thread;
//! - web (`web`, compiled to WebAssembly with Trunk): the page hosts the UI
//!   in a canvas.
//!
//! Each build has exactly one `main`, selected with `cfg(target_arch)`.

mod assembler;
mod bus;
mod cpu;
mod peripherals;
mod regfile;
mod runner;
mod ui;

#[cfg(not(target_arch = "wasm32"))]
mod native;
// Also compiled for tests, so `web::app` is tested by a native `cargo test`
// (only `web::start` needs the browser).
#[cfg(any(target_arch = "wasm32", test))]
mod web;

/// Native entry point: see `native::start`.
///
/// # Errors
///
/// Returns an error if the program file can't be read.
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    native::start()
}

/// Web entry point, called by the JavaScript glue Trunk generates once the
/// page has loaded the wasm module: see `web::start`.
#[cfg(target_arch = "wasm32")]
fn main() {
    web::start();
}

#[cfg(test)]
mod tests {
    use crate::bus::Bus;
    use crate::cpu::exec_one;
    use crate::regfile::RegisterFile;

    /// Loads a hand-encoded program at `MAIN_BASE` and runs it with
    /// `exec_one`, as `CpuRunner` does.
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
