# gecko-sim — Web distribution and `.s` input design

Date: 2026-09-25, revised 2026-09-27.
Status: approved in conversation, not implemented yet.

Revision note (2026-09-27): the first version of this spec rewrote the
threading model (single thread everywhere, a new `Emulator` type replacing
`BoardState`). That was dropped: the goal is now to **change the existing
code as little as possible**. The native build keeps its CPU thread,
`BoardState` and `AtomicU32` exactly as they are; only the body of the CPU
loop moves into a reusable type so the web build can call it once per frame.
The native build also learns to take a `.s` file.

## Goal

Let the author's cs200 classmates run their own lab 1 (game of life)
program in gecko-sim by giving it their **`.s` file directly** (no
Makefile, no `.bin`):

- **Web**: nothing to install, on Linux, Windows and macOS alike. Open a
  URL, drop the `.s` onto the page, the board starts running.
- **Native**: for those who have Rust, `cargo install` from the repo, then
  `gecko-sim program.s` (or `program.bin`).

Success criteria:

- `.s` files are assembled with GNU binutils (same accepted syntax, same
  error messages, same binary as the course toolchain), on both targets.
- Nothing is hosted or paid for by the author beyond a public GitHub repo.
- The author's own lab solution is never published.
- The existing code (CPU thread, `BoardState`, `AtomicU32`, `ui.rs`,
  `Bus`, peripherals, CPU) is kept; changes are additive or pure moves.

## Releases and git history

| Version | What | How to get it |
|---|---|---|
| `v0.1.0` (tag on `539adb2`, already created) | The native app exactly as it was before this work. `.bin` input only. | `cargo install --git <repo> --tag v0.1.0` |
| `v0.2.0` | Native app with `.s` input + web build. | Web page on GitHub Pages, or `cargo install --git <repo> --tag v0.2.0` |

Behavior-preserving refactors (extracting `CpuRunner`) are committed on
`master`, since they stand on their own. The new features happen on a
`web` branch, merged into `master` with
`git merge --no-ff`, then tagged `v0.2.0`. The change of direction is
visible as one branch in `git log --graph`. The web page links to the repo
for those who prefer the native build.

## Decisions

| Topic | Decision |
|---|---|
| Scope | Lab 1 only: the current 4 peripherals and RV32I are enough. |
| Distribution | Static web page on **GitHub Pages**, built and deployed by a GitHub Action on each push to `master`. Native build via `cargo install --git`. |
| Repo | **Public**. `ressources/` was removed from the whole git history (2026-09-25, `git filter-repo`) and stays in `.gitignore`. |
| UI | Same `eframe`/`egui` UI, also compiled to WebAssembly. **`ui.rs` does not change.** |
| Threads | **Native: unchanged** (CPU thread + `Mutex<BoardState>` + `AtomicU32`). **Web: no thread** (`std::thread::spawn` panics on `wasm32-unknown-unknown`); the same batch code runs inside each UI frame. The `Mutex` and `AtomicU32` are kept on the web too: they work single-threaded, so the UI code stays shared. |
| CPU loop | The body of today's CPU-thread loop moves, unchanged, into `CpuRunner::run_batch` (`src/runner.rs`). Native calls it in a loop on its thread, web calls it from the frame. |
| `.s` on native | `main.rs` assembles it with the **local** GNU binutils (`as`, `ld`, `objcopy`), called through `std::process::Command`, before the window opens. Verified 2026-09-27: `riscv64-elf-as` + `ld -T mmio.ld` + `objcopy` on `gol.s` gives a `.bin` byte-identical to the course build. |
| `.s` on web | Dropped onto the page; handled in Rust (egui drop, app states, error display). The assembler itself is GNU binutils compiled to WebAssembly (`as`, `ld`, `objcopy`) from [racerxdl/riscv-online-asm](https://github.com/racerxdl/riscv-online-asm), driven by a small JS file that Rust calls through `wasm-bindgen`. Verified 2026-09-25: byte-identical to GCC on `gol.s`. |
| Build tooling | [Trunk](https://trunkrs.dev/), following [eframe_template](https://github.com/emilk/eframe_template) for the web entry point, `index.html` and the Pages workflow. |

Rejected alternatives: native binaries on GitHub Releases (unsigned-binary
warnings on macOS/Windows), Docker (GUI from a container is painful), a
local web server (still an install), existing Rust assemblers `riscv_asm` /
`lib-rv32-asm` / `rvasm` (tested on `gol.s`: no `/* */` comments, no
constant expressions, no `.section`, no pseudo-instructions beyond
`j`/`ret`), threads on the web (needs nightly Rust, `-Z build-std`, a Web
Worker crate and COOP/COEP headers that GitHub Pages can't set), a
single-threaded rewrite of the native build (first version of this spec:
too many changes to working code), running the binutils `.wasm` files on
native through a wasm runtime (they are Emscripten builds that need their
JS glue).

## Architecture

```
src/
  main.rs         — native entry point (cfg not wasm32). Reads the file given
                    on the command line; assembles it first if it's a .s;
                    then exactly as today: CPU thread + eframe.
  runner.rs       — NEW, shared: CpuRunner (the moved CPU-loop body)
  assembler/
    mod.rs        — NEW: cfg switch between the two backends below
    native.rs     — NEW (cfg not wasm32): local binutils through Command
    web.rs        — NEW (cfg wasm32): wasm-bindgen bridge to web/assemble.js
  web.rs          — NEW (cfg wasm32): web entry point + WebApp (app states
                    wrapped around the unchanged GeckoApp)
  ui.rs, bus.rs, regfile.rs, peripherals/, cpu/   — unchanged
assets/
  mmio.ld         — the course linker script, embedded with include_str! by
                    both assembler backends
web/
  assemble.js     — runs as → ld → objcopy with the binutils wasm files
  binutils/       — vendored riscv64-linux-gnu-{as,ld,objcopy}.{js,wasm} + licenses
index.html        — Trunk entry: canvas + script includes
.github/workflows/pages.yml — tests, assembler check, build with Trunk, deploy
```

### `CpuRunner` (`src/runner.rs`)

A pure move of what the CPU thread closure owns and does today; no new
behavior.

- Fields: `bus: Bus`, `rf: RegisterFile`, `pc: u32`,
  `board_state: Arc<Mutex<BoardState>>`, `pending_presses: Arc<AtomicU32>`.
- `CpuRunner::new(program: &[u8]) -> Result<CpuRunner, RiscvError>`: loads
  the image at `MAIN_BASE`, `pc = MAIN_BASE`, fresh `BoardState` and
  pending-presses mask.
- `board_state()` / `pending_presses()`: clones of the two `Arc`s, to hand
  to `GeckoApp::new` before the runner moves to its thread.
- `run_batch(&mut self) -> Result<(), RiscvError>`: one iteration of
  today's outer `loop`: apply pending presses, run instructions for
  `BATCH_DURATION` (clock checked every `INSTRUCTIONS_BETWEEN_CLOCK_CHECKS`),
  publish the snapshot. Returns the CPU error instead of printing it.
- `pc()`: for error messages.
- `Instant` comes from the [`web-time`](https://crates.io/crates/web-time)
  crate (same API as `std::time::Instant`, which panics on wasm32; plain
  re-export on native).

Native `main.rs` after the change:

```text
let program = <read file, assemble if .s>;
let mut runner = CpuRunner::new(&program)?;
let (board_state, pending_presses) = (runner.board_state(), runner.pending_presses());
thread::spawn(move || loop {
    if let Err(e) = runner.run_batch() { println!("{:?}", e); return; }
});
eframe::run_native(... GeckoApp::new(board_state, pending_presses) ...)
```

### Native `.s` input (`src/assembler/native.rs`)

- `assemble(source: &str) -> Result<Vec<u8>, AssembleError>`.
- Looks for a toolchain by trying these prefixes in order:
  `riscv64-unknown-elf-`, `riscv64-elf-`, `riscv64-linux-gnu-`,
  `riscv32-unknown-elf-`. The first one whose `as --version` runs wins.
- In a temporary directory: writes `program.s` and `mmio.ld`, then runs
  1. `as -march=rv32i -mabi=ilp32 program.s -o program.o`
  2. `ld -m elf32lriscv -T mmio.ld program.o -o program.elf`
  3. `objcopy -O binary program.elf program.bin`
  and reads `program.bin`. `ld` warns that `_start` is missing; harmless,
  the emulator always starts at `0x80000000`.
- Errors: `ToolchainNotFound` (message lists the prefixes tried and says
  to install the RISC-V GNU toolchain or pass a `.bin`); `ToolFailed`
  (the tool's stderr, verbatim); I/O errors.
- `main.rs` picks the path by extension: `.s` → assemble, `.bin` → load
  as today, anything else → usage error. Assembly errors are printed to
  stderr and the process exits with a non-zero code, before any window
  opens.

### Web entry point (`src/web.rs`)

`WebApp` implements `eframe::App` and wraps the unchanged `GeckoApp`:

- **Waiting for a program**: a `GeckoApp` built on a fresh, never-updated
  `BoardState` (board drawn, off), plus a "Drop your `.s` (or `.bin`) file
  here" message.
- **Assembling**: `.s` dropped, assembler running (see below).
- **Assembler error**: binutils output (stderr) shown verbatim in a panel,
  board off. Dropping a fixed file retries.
- **Running**: holds a `CpuRunner` and the `GeckoApp` built from its
  `Arc`s. Each frame: call `run_batch()` repeatedly until
  `WEB_CPU_BUDGET_PER_FRAME` (starting point 8 ms, tuned during
  implementation) is spent, then delegate drawing to `GeckoApp::ui`.
  `GeckoApp` already asks for a repaint every 16 ms.
- **CPU stopped**: `run_batch` returned an error: the error and `pc` are
  shown in a panel, the board keeps its last state, the runner is dropped.

Dropping a new file at any point starts over with a fresh `CpuRunner`:
that's the reset. Dropped files come from
`ctx.input(|i| i.raw.dropped_files.clone())` (bytes on the web).
Extension `.s` → assembler, `.bin` → `CpuRunner::new` directly, anything
else → "Unsupported file: expected .s or .bin".

### Web assembler bridge (`src/assembler/web.rs` + `web/assemble.js`)

- `web/assemble.js` exposes `assemble(source: string) -> Promise<Uint8Array>`
  (rejects with the binutils output as text). It runs the same
  `as` → `ld` → `objcopy` pipeline as the native backend, each tool in its
  own Emscripten instance with an in-memory filesystem, with `mmio.ld`
  passed in from Rust.
- `src/assembler/web.rs` declares it with `wasm-bindgen` and awaits the
  promise with `wasm-bindgen-futures::spawn_local`. The result goes back to
  `WebApp` through a `std::sync::mpsc` channel that it polls each frame
  (egui has no async).
- The binutils files (~7 MB) are only fetched on the first `.s` drop, then
  cached by the browser. Everything runs locally in the student's browser:
  no code is uploaded anywhere.

## Licensing

- binutils is GPLv3. On the web it is shipped as separate programs
  (aggregation), so gecko-sim's own license is unaffected, but the page
  must ship the GPLv3 text and point to the corresponding source (binutils
  2.46 release + racerxdl's `build.sh`). The racerxdl wrapper is MIT: keep
  its notice. On native, binutils is the user's own install: nothing to
  ship.
- gecko-sim needs a license of its own before going public (the vendored
  `lib-rv32` code is MIT, so MIT is the natural choice). Author's call.
- `mmio.ld` (the short course template linker script) is copied into
  `assets/` so the build doesn't depend on the ignored `ressources/`
  folder. It contains no solution code.

## Error handling summary

| Failure | Native | Web |
|---|---|---|
| `.s` doesn't assemble / link | binutils stderr on the terminal, exit code ≠ 0 | binutils output, verbatim, in a panel |
| No RISC-V toolchain installed | "no RISC-V toolchain found" + prefixes tried | n/a |
| File is neither `.s` nor `.bin` | usage error | "Unsupported file: expected .s or .bin" |
| `.bin` too big for main RAM | load error | load error in a panel |
| CPU error at runtime | printed, CPU thread stops, window stays (unchanged) | error + `pc` in a panel, board frozen |
| binutils wasm fails to load | n/a | "Could not load the assembler" + the underlying error |

## Testing

- **Existing tests** stay green and unchanged: `Bus`, peripherals, CPU,
  `ui.rs` are untouched.
- **`CpuRunner` unit tests** (headless): `new` loads a hand-encoded program
  at `MAIN_BASE`; `run_batch` publishes LED writes to the `BoardState`;
  a pending press is applied before the batch runs; a CPU error is returned
  (not printed) with the runner's `pc` at the faulting instruction; a
  program too big for main RAM is rejected by `new`.
- **Native assembler tests** (need a RISC-V toolchain, installed in CI):
  a tiny `.s` assembles to the expected machine code; a syntax error gives
  `ToolFailed` with binutils' message; a program using `.section`, `/* */`
  comments and pseudo-instructions assembles (the features the Rust
  assemblers lacked).
- **Assembler parity, in CI**: a small public test program (written for the
  test, e.g. lights a few LEDs; **not** `gol.s`) is assembled by the native
  backend and by `web/assemble.js` under Node; the two `.bin` files must be
  byte-identical.
- **Manual check before each release**: native `gecko-sim gol.s`; web page
  on at least two browsers, drop `gol.s` locally (never committed), run the
  `freeze-after-speeding-up.md` repro steps.

## Out of scope

- Other cs200 labs, other peripherals, RV32M/C.
- Dropping files onto the native window (native input stays the command
  line argument).
- Debugging features (unchanged from the original design).
- Offline/PWA install.

## Impact on existing docs

- `CLAUDE.md`, "UI decision": amended — still `eframe`/`egui`, now also
  built for the web; native threading unchanged, web runs the CPU inside
  the frame.
- `TODO.md` gets a new section for this work (pedagogical, no code),
  following the usual mode: the assistant writes the tests, the author
  writes the code.
