//! The web app's logic: loading a dropped program and running the CPU.
//!
//! `WebApp` wraps the shared `GeckoApp` (which draws the board) and does
//! what the native entry point does with the command line and the CPU
//! thread: here a program arrives by drag and drop, and the CPU runs
//! inside each frame, for `CPU_BUDGET_PER_FRAME`, since the browser has no
//! threads.
//!
//! ```text
//! Waiting ──drop a .bin──► Running(CpuRunner) ──CPU error──► Error(msg)
//!    └──bad file──► Error(msg)          ▲                      │
//!                                       └──── drop a file ─────┘
//! ```
//!
//! Dropping a file always starts over with a fresh CPU: that's the reset.

use std::{
    path::Path,
    sync::{Arc, Mutex, atomic::AtomicU32},
    time::Duration,
};

use crate::{
    runner::CpuRunner,
    ui::{BoardState, GeckoApp},
};
use web_time::Instant;

/// How long the CPU runs in each frame, before the board is drawn.
const CPU_BUDGET_PER_FRAME: Duration = Duration::from_millis(8);

/// Where the app is in its lifecycle.
enum State {
    /// No program yet.
    Waiting,
    /// A program is loaded; the CPU runs every frame.
    Running(CpuRunner),
    /// Loading failed or the CPU stopped: the text to show.
    Error(String),
}

/// The `eframe` app run in the browser.
pub struct WebApp {
    state: State,
    /// Draws the board; rebuilt on each load, wired to the new CPU.
    board: GeckoApp,
}

impl WebApp {
    /// Creates the app waiting for a file, with the board off.
    pub fn new() -> Self {
        Self {
            state: State::Waiting,
            board: GeckoApp::new(
                Arc::new(Mutex::new(BoardState::new())),
                Arc::new(AtomicU32::new(0)),
            ),
        }
    }

    /// Starts over with the dropped file: runs it if it's a valid program,
    /// shows why otherwise.
    fn load(&mut self, file_name: &str, bytes: &[u8]) {
        match Self::runner_for(file_name, bytes) {
            Ok(runner) => {
                self.board = GeckoApp::new(runner.board_state(), runner.pending_presses());
                self.state = State::Running(runner);
            }
            Err(message) => self.state = State::Error(message),
        }
    }

    /// Runs the loaded program for about `budget` (at least one batch).
    /// On a CPU error, stops it and keeps the error and its `pc` to show;
    /// the board keeps its last picture.
    fn run_cpu(&mut self, budget: Duration) {
        if let State::Running(runner) = &mut self.state {
            let start = Instant::now();
            loop {
                if let Err(e) = runner.run_batch() {
                    self.state =
                        State::Error(format!("CPU stopped at pc 0x{:08x}: {e:?}", runner.pc()));
                    return;
                }
                if start.elapsed() >= budget {
                    break;
                }
            }
        }
    }

    /// The text to show above the board, if any.
    fn message(&self) -> Option<String> {
        match &self.state {
            State::Waiting => Some("Drop a .s or .bin file here".to_string()),
            State::Running(_) => None,
            State::Error(m) => Some(m.clone()),
        }
    }

    /// Turns a dropped file into a ready-to-run CPU, or explains why it can't.
    fn runner_for(file_name: &str, bytes: &[u8]) -> Result<CpuRunner, String> {
        match Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
        {
            Some("bin") => {
                CpuRunner::new(bytes).map_err(|e| format!("cannot load {file_name}: {e:?}"))
            }
            _ => Err(format!(
                "unsupported file: {file_name} (expected a .s or .bin file)"
            )),
        }
    }
}

impl eframe::App for WebApp {
    /// Draws the message (if any) above the board.
    fn ui(&mut self, ui: &mut eframe::egui::Ui, frame: &mut eframe::Frame) {
        if let Some(msg) = self.message() {
            ui.label(msg);
        }
        self.board.ui(ui, frame);
    }

    /// Called by eframe before each `ui`: loads the files dropped since the
    /// last frame, then runs the CPU for this frame's time budget.
    fn logic(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        // Files dropped on the page since the last frame. On the web, egui
        // gives their content in `bytes` (there's no path to read from).
        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());
        for file in dropped_files {
            if let Some(bytes) = file.bytes {
                self.load(&file.name, &bytes);
            }
        }

        self.run_cpu(CPU_BUDGET_PER_FRAME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// Turns hand-encoded instructions into the bytes of a `.bin` image.
    fn image(program: &[u32]) -> Vec<u8> {
        program.iter().flat_map(|i| i.to_le_bytes()).collect()
    }

    /// Writes 0x3f to `SEVEN_SEGS`, then loops forever.
    const WRITE_SEVEN_SEGS: [u32; 4] = [0x600000b7, 0x03f00113, 0x0020a023, 0x0000006f];

    /// `addi x1, x0, 1`, then zeroed RAM: invalid opcode at 0x80000004.
    const CRASHES_AT_SECOND_INSTRUCTION: [u32; 1] = [0x00100093];

    const BUDGET: Duration = Duration::from_millis(5);

    /// The 7-segment value the running program last published.
    fn seven_segs(app: &WebApp) -> u32 {
        match &app.state {
            State::Running(runner) => runner.board_state().lock().unwrap().seven_segs,
            _ => panic!("expected a running program"),
        }
    }

    #[test]
    fn starts_waiting_for_a_file() {
        let app = WebApp::new();

        assert!(matches!(app.state, State::Waiting));
        let message = app.message().expect("the waiting state shows a message");
        assert!(message.contains(".bin"), "got: {message}");
    }

    #[test]
    fn dropping_a_bin_runs_it() {
        let mut app = WebApp::new();

        app.load("prog.bin", &image(&WRITE_SEVEN_SEGS));
        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Running(_)));
        assert_eq!(app.message(), None, "nothing is shown over a running board");
        assert_eq!(seven_segs(&app), 0x3f);
    }

    #[test]
    fn unsupported_extension_is_reported() {
        let mut app = WebApp::new();

        app.load("notes.txt", b"hello");

        let message = app.message().expect("an error is shown");
        assert!(message.contains("unsupported file"), "got: {message}");
        assert!(message.contains("notes.txt"), "got: {message}");
    }

    #[test]
    fn a_bin_too_big_for_ram_is_reported() {
        let mut app = WebApp::new();

        app.load("huge.bin", &vec![0u8; 0x100000 + 1]);

        assert!(matches!(app.state, State::Error(_)));
        let message = app.message().expect("an error is shown");
        assert!(message.contains("huge.bin"), "got: {message}");
    }

    #[test]
    fn a_cpu_error_stops_the_program_and_shows_the_pc() {
        let mut app = WebApp::new();
        app.load("crash.bin", &image(&CRASHES_AT_SECOND_INSTRUCTION));

        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Error(_)));
        let message = app.message().expect("the error is shown");
        assert!(message.contains("0x80000004"), "got: {message}");

        // Nothing left to run: further frames must not panic.
        app.run_cpu(BUDGET);
        assert!(matches!(app.state, State::Error(_)));
    }

    #[test]
    fn dropping_a_new_file_starts_over() {
        let mut app = WebApp::new();
        app.load("crash.bin", &image(&CRASHES_AT_SECOND_INSTRUCTION));
        app.run_cpu(BUDGET);

        app.load("prog.bin", &image(&WRITE_SEVEN_SEGS));
        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Running(_)));
        assert_eq!(seven_segs(&app), 0x3f);
    }
}
