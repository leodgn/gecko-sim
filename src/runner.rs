//! The CPU side of the emulator: the machine state and one batch of
//! execution.
//!
//! `CpuRunner` owns the `Bus`, the register file and the `pc`. Each call to
//! `run_batch` applies the button presses recorded by the UI, executes
//! instructions for `BATCH_DURATION`, then publishes the LEDs and the
//! 7-segment display to the shared `BoardState`.
//!
//! It only knows how to run one batch, not when: the native build calls
//! `run_batch` in a loop on a dedicated thread.

use crate::bus::{Bus, MAIN_BASE};
use crate::cpu::{RiscvError, exec_one};
use crate::regfile::RegisterFile;
use crate::ui::BoardState;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long one `run_batch` call executes before publishing the board state.
const BATCH_DURATION: Duration = Duration::from_millis(3);
/// How many instructions run between two checks of the batch clock.
const INSTRUCTIONS_BETWEEN_CLOCK_CHECKS: u32 = 1_000;

/// A loaded program, ready to run, and the handles shared with the UI.
pub struct CpuRunner {
    bus: Bus,
    rf: RegisterFile,
    pc: u32,
    board_state: Arc<Mutex<BoardState>>,
    pending_presses: Arc<AtomicU32>,
}

impl CpuRunner {
    /// Loads `program` (a raw `.bin` image) at `MAIN_BASE`, with `pc` pointing
    /// at its first instruction and the board off.
    ///
    /// # Errors
    ///
    /// Returns `MemoryOutOfBoundsError` if the program doesn't fit in main RAM.
    pub fn new(program: &[u8]) -> Result<Self, RiscvError> {
        let mut bus = Bus::new();
        bus.store_byte(MAIN_BASE, program)?;

        Ok(Self {
            bus,
            rf: RegisterFile::new(),
            pc: MAIN_BASE,
            board_state: Arc::new(Mutex::new(BoardState::new())),
            pending_presses: Arc::new(AtomicU32::new(0)),
        })
    }

    /// Returns the address of the next instruction to execute. After a CPU
    /// error, the address of the instruction that failed.
    pub fn pc(&self) -> u32 {
        self.pc
    }

    /// Returns a handle to the board state published after each batch.
    pub fn board_state(&self) -> Arc<Mutex<BoardState>> {
        Arc::clone(&self.board_state)
    }

    /// Returns a handle to the pending button presses: one bit per button,
    /// numbered as in the `BUTTONS` register. The UI sets bits, the next
    /// batch applies and clears them.
    pub fn pending_presses(&self) -> Arc<AtomicU32> {
        Arc::clone(&self.pending_presses)
    }

    /// Runs one batch: applies the pending button presses, executes
    /// instructions for `BATCH_DURATION`, then publishes the board state.
    ///
    /// # Errors
    ///
    /// Returns the CPU error (invalid instruction, bad memory access) and
    /// stops right away, without publishing. `pc` stays on the faulting
    /// instruction.
    pub fn run_batch(&mut self) -> Result<(), RiscvError> {
        let pressed = self.pending_presses.swap(0, Ordering::AcqRel);
        for bit in 0..10u8 {
            if pressed & (1 << bit) != 0 {
                self.bus.press_button(bit);
            }
        }

        let batch_start = Instant::now();
        loop {
            for _ in 0..INSTRUCTIONS_BETWEEN_CLOCK_CHECKS {
                exec_one(&mut self.pc, &mut self.bus, &mut self.rf)?;
            }
            if batch_start.elapsed() >= BATCH_DURATION {
                break;
            }
        }

        // Publish a fresh snapshot for the UI to draw.
        let mut state = self.board_state.lock().unwrap();
        state.leds_red = self.bus.leds().red();
        state.leds_green = self.bus.leds().green();
        state.leds_blue = self.bus.leds().blue();
        state.seven_segs = self.bus.seven_segs();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::MAIN_BASE;
    use crate::cpu::RiscvError;
    use std::sync::atomic::Ordering;

    /// Turns hand-encoded instructions into the little-endian bytes of a
    /// `.bin` image.
    fn image(program: &[u32]) -> Vec<u8> {
        program.iter().flat_map(|i| i.to_le_bytes()).collect()
    }

    /// `lui x1, 0x60000` / `addi x2, x0, 0x3f` / `sw x2, 0(x1)` / `j .`
    /// Writes 0x3f to `SEVEN_SEGS`, then loops forever.
    const WRITE_SEVEN_SEGS: [u32; 4] = [0x600000b7, 0x03f00113, 0x0020a023, 0x0000006f];

    /// `lui x1, 0x60000` / `lui x3, 0x70000` /
    /// `loop: lw x2, 4(x3)` / `sw x2, 0(x1)` / `j loop`
    /// Copies `BUTTONS` into `SEVEN_SEGS`, forever.
    const ECHO_BUTTONS: [u32; 5] = [0x600000b7, 0x700001b7, 0x0041a103, 0x0020a023, 0xff9ff06f];

    #[test]
    fn new_starts_at_main_base_with_the_board_off() {
        let mut runner = CpuRunner::new(&image(&WRITE_SEVEN_SEGS)).unwrap();

        assert_eq!(runner.pc(), MAIN_BASE);
        let state = runner.board_state();
        let state = state.lock().unwrap();
        assert_eq!(
            state.seven_segs, 0,
            "nothing is published before the first batch"
        );
        assert_eq!(runner.pending_presses().load(Ordering::Acquire), 0);
    }

    #[test]
    fn new_rejects_a_program_bigger_than_main_ram() {
        let too_big = vec![0u8; 0x100000 + 1];

        assert!(CpuRunner::new(&too_big).is_err());
    }

    #[test]
    fn run_batch_publishes_the_board_state() {
        let mut runner = CpuRunner::new(&image(&WRITE_SEVEN_SEGS)).unwrap();
        let state = runner.board_state();

        runner.run_batch().unwrap();

        assert_eq!(
            state.lock().unwrap().seven_segs,
            0x3f,
            "the Arc handed out before the batch must see the published snapshot"
        );
    }

    #[test]
    fn run_batch_applies_pending_presses_first() {
        let mut runner = CpuRunner::new(&image(&ECHO_BUTTONS)).unwrap();
        let presses = runner.pending_presses();
        let state = runner.board_state();

        // BUTTON_0 is bit 6, as `GeckoApp` records it.
        presses.fetch_or(1 << 6, Ordering::AcqRel);
        runner.run_batch().unwrap();

        assert_eq!(state.lock().unwrap().seven_segs, 1 << 6);
        assert_eq!(
            presses.load(Ordering::Acquire),
            0,
            "applied presses must be removed from the pending mask"
        );
    }

    #[test]
    fn run_batch_returns_cpu_errors_with_the_faulting_pc() {
        // `addi x1, x0, 1`, then zeroed RAM: 0x00000000 is not a valid opcode.
        let mut runner = CpuRunner::new(&image(&[0x00100093])).unwrap();

        let result = runner.run_batch();

        assert!(
            matches!(result, Err(RiscvError::InvalidOpcodeError(..))),
            "got {:?}",
            result
        );
        assert_eq!(runner.pc(), MAIN_BASE + 4);
    }
}
