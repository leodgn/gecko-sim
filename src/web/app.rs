//! The web app's logic: loading a dropped program and running the CPU.
//!
//! `WebApp` wraps the shared `GeckoApp` (which draws the board) and does
//! what the native entry point does with the command line and the CPU
//! thread: here a program arrives by drag and drop, and the CPU runs
//! inside each frame, for `CPU_BUDGET_PER_FRAME`, since the browser has no
//! threads.
//!
//! ```text
//! Waiting ──drop a .s──► Assembling ──binary──► Running ──CPU error──► Error
//!    │                        └──binutils' messages──────────────────► Error
//!    ├──drop a .bin───────────────────────────► Running
//!    └──unsupported file─────────────────────────────────────────────► Error
//! ```
//!
//! Assembling is asynchronous (it runs in JavaScript, see
//! `assembler::web`): `load` starts it, and `poll_assembly` checks each
//! frame whether the result has arrived. The assembler is a parameter of
//! `WebApp::new`, so tests can use fake ones.
//!
//! Dropping a file always starts over with a fresh CPU: that's the reset.

use std::{
    path::Path,
    str::from_utf8,
    sync::{
        Arc, Mutex,
        atomic::AtomicU32,
        mpsc::{Receiver, TryRecvError},
    },
    time::Duration,
};

use crate::{
    runner::CpuRunner,
    ui::{BoardState, GeckoApp},
};
use web_time::Instant;

/// How long the CPU runs in each frame, before the board is drawn.
const CPU_BUDGET_PER_FRAME: Duration = Duration::from_millis(8);

/// Starts assembling `(file name, source)` and returns right away; the
/// binary image, or the assembler's messages, arrive in the receiver later.
pub type Assembler = fn(&str, &str) -> Receiver<Result<Vec<u8>, String>>;

/// Where the app is in its lifecycle.
// There's a single `State` per app: the size of `Running` doesn't matter.
#[allow(clippy::large_enum_variant)]
enum State {
    /// No program yet.
    Waiting,
    /// A program is loaded; the CPU runs every frame.
    Running(CpuRunner),
    /// A dropped `.s` is being assembled; the result arrives in `result`.
    Assembling {
        file_name: String,
        result: Receiver<Result<Vec<u8>, String>>,
    },
    /// Loading failed or the CPU stopped: the text to show.
    Error(String),
}

/// The `eframe` app run in the browser.
pub struct WebApp {
    state: State,
    /// Draws the board; rebuilt on each load, wired to the new CPU.
    board: GeckoApp,
    /// Assembles dropped `.s` files.
    assembler: Assembler,
}

impl WebApp {
    /// Creates the app waiting for a file, with the board off. `assembler`
    /// is used for dropped `.s` files.
    pub fn new(assembler: Assembler) -> Self {
        Self {
            state: State::Waiting,
            board: GeckoApp::new(
                Arc::new(Mutex::new(BoardState::new())),
                Arc::new(AtomicU32::new(0)),
            ),
            assembler,
        }
    }

    /// Starts over with the dropped file: runs it if it's a valid program,
    /// shows why otherwise.
    fn load(&mut self, file_name: &str, bytes: &[u8]) {
        match Path::new(file_name)
            .extension()
            .and_then(|ext| ext.to_str())
        {
            Some("bin") => self.start(
                CpuRunner::new(bytes).map_err(|e| format!("cannot load {file_name}: {e:?}")),
            ),
            Some("s") => self.start_assembling(file_name, bytes),
            _ => {
                self.state = State::Error(format!(
                    "unsupported file: {file_name} (expected a .s or .bin file)"
                ))
            }
        }
    }

    /// Runs `runner` on a fresh board if loading worked, shows why
    /// otherwise.
    fn start(&mut self, runner: Result<CpuRunner, String>) {
        match runner {
            Ok(runner) => {
                self.board = GeckoApp::new(runner.board_state(), runner.pending_presses());
                self.state = State::Running(runner);
            }
            Err(message) => self.state = State::Error(message),
        }
    }

    /// Starts assembling a dropped `.s`; `poll_assembly` picks up the
    /// result.
    fn start_assembling(&mut self, file_name: &str, bytes: &[u8]) {
        self.state = match from_utf8(bytes) {
            Ok(source) => State::Assembling {
                file_name: file_name.to_string(),
                result: (self.assembler)(file_name, source),
            },
            Err(_) => State::Error(format!("{file_name} is not a text file")),
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
            State::Assembling { file_name, .. } => Some(format!("Assembling {file_name}...")),
            State::Running(_) => None,
            State::Error(m) => Some(m.clone()),
        }
    }

    /// While assembling, checks (without waiting) whether the result has
    /// arrived: runs the program, or shows binutils' messages verbatim.
    fn poll_assembly(&mut self) {
        let State::Assembling { file_name, result } = &self.state else {
            return;
        };
        match result.try_recv() {
            Ok(Ok(bin)) => self
                .start(CpuRunner::new(&bin).map_err(|e| format!("cannot load {file_name}: {e:?}"))),
            Ok(Err(msg)) => self.state = State::Error(msg),
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => {
                self.state = State::Error(format!(
                    "the assembler stopped without an answer for {file_name}"
                ))
            }
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
    /// last frame, picks up a finished assembly, then runs the CPU for this
    /// frame's time budget.
    fn logic(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        // Files dropped on the page since the last frame. On the web, egui
        // gives their content in `bytes` (there's no path to read from).
        let dropped_files = ctx.input(|input| input.raw.dropped_files.clone());
        for file in dropped_files {
            if let Some(bytes) = file.bytes {
                self.load(&file.name, &bytes);
            }
        }
        self.poll_assembly();
        self.run_cpu(CPU_BUDGET_PER_FRAME);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc::{Receiver, channel};
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

    // Fake assemblers: `WebApp` takes the assembler as a parameter, so the
    // tests don't need the browser (the real one is `assembler::web`).

    /// For tests that never drop a `.s`.
    fn no_assembler(_: &str, _: &str) -> Receiver<Result<Vec<u8>, String>> {
        panic!("the assembler must not be called");
    }

    /// Answers right away with a program that writes 0x3f to `SEVEN_SEGS`.
    fn assembles_seven_segs(_: &str, _: &str) -> Receiver<Result<Vec<u8>, String>> {
        let (sender, receiver) = channel();
        sender.send(Ok(image(&WRITE_SEVEN_SEGS))).unwrap();
        receiver
    }

    /// Fails with what it was given, to check what `WebApp` passes it.
    fn echoes_its_input(file_name: &str, source: &str) -> Receiver<Result<Vec<u8>, String>> {
        let (sender, receiver) = channel();
        sender.send(Err(format!("{file_name}|{source}"))).unwrap();
        receiver
    }

    /// Fails like binutils does on a syntax error.
    fn syntax_error(_: &str, _: &str) -> Receiver<Result<Vec<u8>, String>> {
        let (sender, receiver) = channel();
        sender.send(Err(BINUTILS_ERROR.to_string())).unwrap();
        receiver
    }
    const BINUTILS_ERROR: &str =
        "gol.s: Assembler messages:\ngol.s:1: Error: illegal operands `addi x1,x0'\n";

    /// Never answers, like an assembler that's still working.
    fn still_working(_: &str, _: &str) -> Receiver<Result<Vec<u8>, String>> {
        let (sender, receiver) = channel();
        // Keep the sender alive, so the channel stays open and empty.
        std::mem::forget(sender);
        receiver
    }

    /// Stops without answering (e.g. the JavaScript side crashed).
    fn gives_up(_: &str, _: &str) -> Receiver<Result<Vec<u8>, String>> {
        let (_sender, receiver) = channel();
        receiver
    }

    /// The 7-segment value the running program last published.
    fn seven_segs(app: &WebApp) -> u32 {
        match &app.state {
            State::Running(runner) => runner.board_state().lock().unwrap().seven_segs,
            _ => panic!("expected a running program"),
        }
    }

    #[test]
    fn starts_waiting_for_a_file() {
        let app = WebApp::new(no_assembler);

        assert!(matches!(app.state, State::Waiting));
        let message = app.message().expect("the waiting state shows a message");
        assert!(message.contains(".bin"), "got: {message}");
    }

    #[test]
    fn dropping_a_bin_runs_it() {
        let mut app = WebApp::new(no_assembler);

        app.load("prog.bin", &image(&WRITE_SEVEN_SEGS));
        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Running(_)));
        assert_eq!(app.message(), None, "nothing is shown over a running board");
        assert_eq!(seven_segs(&app), 0x3f);
    }

    #[test]
    fn unsupported_extension_is_reported() {
        let mut app = WebApp::new(no_assembler);

        app.load("notes.txt", b"hello");

        let message = app.message().expect("an error is shown");
        assert!(message.contains("unsupported file"), "got: {message}");
        assert!(message.contains("notes.txt"), "got: {message}");
    }

    #[test]
    fn a_bin_too_big_for_ram_is_reported() {
        let mut app = WebApp::new(no_assembler);

        app.load("huge.bin", &vec![0u8; 0x100000 + 1]);

        assert!(matches!(app.state, State::Error(_)));
        let message = app.message().expect("an error is shown");
        assert!(message.contains("huge.bin"), "got: {message}");
    }

    #[test]
    fn a_cpu_error_stops_the_program_and_shows_the_pc() {
        let mut app = WebApp::new(no_assembler);
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
        let mut app = WebApp::new(no_assembler);
        app.load("crash.bin", &image(&CRASHES_AT_SECOND_INSTRUCTION));
        app.run_cpu(BUDGET);

        app.load("prog.bin", &image(&WRITE_SEVEN_SEGS));
        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Running(_)));
        assert_eq!(seven_segs(&app), 0x3f);
    }

    #[test]
    fn dropping_a_s_starts_assembling_it() {
        let mut app = WebApp::new(still_working);

        app.load("gol.s", b"addi x1, x0, 5\n");

        assert!(matches!(app.state, State::Assembling { .. }));
        let message = app.message().expect("assembling shows a message");
        assert!(message.contains("Assembling"), "got: {message}");
        assert!(message.contains("gol.s"), "got: {message}");
    }

    #[test]
    fn keeps_waiting_until_the_assembler_answers() {
        let mut app = WebApp::new(still_working);
        app.load("gol.s", b"addi x1, x0, 5\n");

        app.poll_assembly();
        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Assembling { .. }));
    }

    #[test]
    fn an_assembled_program_runs() {
        let mut app = WebApp::new(assembles_seven_segs);
        app.load("prog.s", b"(source)");

        app.poll_assembly();
        app.run_cpu(BUDGET);

        assert!(matches!(app.state, State::Running(_)));
        assert_eq!(seven_segs(&app), 0x3f);
    }

    #[test]
    fn the_assembler_gets_the_file_name_and_the_source() {
        let mut app = WebApp::new(echoes_its_input);
        app.load("prog.s", b"addi x1, x0, 5\n");

        app.poll_assembly();

        assert_eq!(app.message().as_deref(), Some("prog.s|addi x1, x0, 5\n"));
    }

    #[test]
    fn assembler_errors_are_shown_verbatim() {
        let mut app = WebApp::new(syntax_error);
        app.load("gol.s", b"addi x1, x0\n");

        app.poll_assembly();

        assert!(matches!(app.state, State::Error(_)));
        assert_eq!(app.message().as_deref(), Some(BINUTILS_ERROR));
    }

    #[test]
    fn an_assembler_that_gives_up_is_reported() {
        let mut app = WebApp::new(gives_up);
        app.load("gol.s", b"addi x1, x0, 5\n");

        app.poll_assembly();

        assert!(matches!(app.state, State::Error(_)));
        let message = app.message().expect("an error is shown");
        assert!(message.contains("assembler"), "got: {message}");
    }

    #[test]
    fn a_s_file_that_is_not_text_is_reported() {
        let mut app = WebApp::new(no_assembler);

        app.load("gol.s", &[0xff, 0xfe, 0x00]);

        assert!(matches!(app.state, State::Error(_)));
        let message = app.message().expect("an error is shown");
        assert!(message.contains("gol.s"), "got: {message}");
    }

    #[test]
    fn dropping_a_file_while_assembling_replaces_it() {
        let mut app = WebApp::new(assembles_seven_segs);
        app.load("gol.s", b"(source)");

        app.load("crash.bin", &image(&CRASHES_AT_SECOND_INSTRUCTION));
        app.poll_assembly();

        assert!(
            matches!(app.state, State::Running(_)),
            "the .bin replaced the .s: the assembler's answer must be ignored"
        );
    }
}
