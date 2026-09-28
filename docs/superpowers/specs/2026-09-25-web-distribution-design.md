# gecko-sim — Web distribution design

Date: 2026-09-25
Status: approved in conversation, not implemented yet.

## Goal

Let the author's cs200 classmates run their own lab 1 (game of life)
program in gecko-sim with **nothing to install**, on Linux, Windows and
macOS alike, by giving it their **`.s` file directly** (no toolchain, no
Makefile, no `.bin`).

Success criteria:

- A classmate opens a URL, drops their `.s` onto the page, and the board
  starts running their program.
- The `.s` is assembled with the exact same toolchain behavior as the
  course (GNU binutils): same accepted syntax, same error messages, same
  binary.
- Nothing is hosted or paid for by the author beyond a public GitHub repo.
- The author's own lab solution is never published.

## Decisions

| Topic | Decision |
|---|---|
| Scope | Lab 1 only: the current 4 peripherals and RV32I are enough. |
| Distribution | Static web page on **GitHub Pages**, built and deployed by a GitHub Action on each push to the default branch. |
| Repo | **Public**. `ressources/` was removed from the whole git history (2026-09-25, `git filter-repo`) and stays in `.gitignore`. |
| UI | Same `eframe`/`egui` UI, compiled to WebAssembly. The drawing code in `ui.rs` does not change. |
| Threads | **Single thread.** The CPU runs for a fixed time budget inside each UI frame. The CPU thread, the `Mutex<BoardState>` and the `AtomicU32` are removed. Same code path for native and web. |
| Assembler | GNU binutils compiled to WebAssembly (`as`, `ld`, `objcopy`), taken from [racerxdl/riscv-online-asm](https://github.com/racerxdl/riscv-online-asm). Verified: assembling + linking `gol.s` with `mmio.ld` gives a `.bin` **byte-identical** to `riscv64-elf-gcc` + `objcopy`. |
| Native build | Kept for development: `cargo run -- program.bin` still works. `.s` input is web-only. |
| Build tooling | [Trunk](https://trunkrs.dev/), following [eframe_template](https://github.com/emilk/eframe_template) for the web entry point, `index.html` and the Pages workflow. |

Rejected alternatives (see conversation): native binaries on GitHub
Releases (unsigned-binary warnings on macOS/Windows, `.s` input would depend
on each student's local toolchain), `cargo install` (needs Rust), Docker
(GUI from a container is painful), a local web server (still an install),
existing Rust assemblers `riscv_asm` / `lib-rv32-asm` / `rvasm` (tested on
`gol.s`: no `/* */` comments, no constant expressions, no `.section`, no
pseudo-instructions beyond `j`/`ret`), a Web Worker for the CPU (complexity
for no benefit at this compute volume).

## Architecture

```
src/
  main.rs         — native entry point (cfg not wasm32): reads the .bin, runs eframe
  web.rs          — web entry point (cfg wasm32): starts eframe::WebRunner on the canvas
  emulator.rs     — NEW: Emulator = Bus + RegisterFile + pc + stop state
  assembler.rs    — NEW (wasm32 only): Rust side of the JS assembler bridge
  ui.rs           — GeckoApp: owns the Emulator, drives it each frame, draws
  bus.rs, regfile.rs, peripherals/, cpu/   — unchanged
web/
  assemble.js     — runs as → ld → objcopy with the binutils wasm files
  binutils/       — vendored riscv64-linux-gnu-{as,ld,objcopy}.{js,wasm} + licenses
  mmio.ld         — the course linker script, embedded in the page
index.html        — Trunk entry: canvas + script includes
.github/workflows/pages.yml — build with Trunk, deploy to GitHub Pages
```

### `Emulator` (new, headless, fully unit-testable)

Owns everything the CPU thread owned today:

- `Emulator::new(program: &[u8]) -> Result<Emulator, LoadError>` — loads the
  image at `MAIN_BASE`, `pc = MAIN_BASE`.
- `run_for(&mut self, budget: Duration)` — executes instructions until the
  budget is spent (clock checked every N instructions, as today). Does
  nothing once stopped.
- `press_button(&mut self, bit: u8)`.
- `leds()`, `seven_segs()` — read-only views for drawing.
- `stopped(&self) -> Option<&StopReason>` — set when `exec_one` returns an
  error; keeps the error and the `pc` where it happened.

`BoardState` disappears: the UI reads straight from the `Emulator`, since
both now live on the same thread.

### Frame loop (`GeckoApp::ui`)

1. Apply clicks from the previous frame (`press_button`), as today.
2. `emulator.run_for(CPU_BUDGET_PER_FRAME)` (starting point: 8 ms).
3. Draw the board from `emulator`.
4. `request_repaint()` so the next frame comes right away.

Speed: in a release build, 8 ms per frame is far more than `gol.s` needs
(the current debug build already runs it comfortably with one thread
busy-looping). The exact budget is tuned during implementation.

### App states

- **Waiting for a program**: the board is drawn, off, with a "Drop your
  `.s` (or `.bin`) file here" message. Native builds skip this state (the
  program comes from the command line).
- **Assembling**: `.s` dropped, assembler running (async, see below).
- **Running**: the emulator runs every frame.
- **Assembler error**: the binutils output (stderr) is shown verbatim in a
  panel, the board stays off. Dropping a fixed file retries.
- **CPU stopped**: the error and its `pc` are shown in a panel, the board
  keeps its last state. (Today's `println!` is replaced by this.)

Dropping a new file at any point restarts from scratch with a fresh
`Emulator`: that's the "reset" button.

### Assembler bridge (web only)

- egui already gives dropped files' bytes on the web
  (`ctx.input(|i| i.raw.dropped_files)`). `.bin` → loaded directly. `.s` →
  sent to the assembler.
- `web/assemble.js` exposes `assemble(source: string) -> Promise<Uint8Array>`
  (rejects with the binutils output as text). It runs, each in its own
  Emscripten instance with an in-memory filesystem:
  1. `as -march=rv32i -mabi=ilp32 file.s -o file.o`
  2. `ld -m elf32lriscv -T mmio.ld file.o -o file.elf`
  3. `objcopy -O binary file.elf file.bin`
  This is the exact pipeline validated by the 2026-09-25 spike (output
  byte-identical to GCC on `gol.s`). Note: `ld` warns that `_start` is
  missing; harmless, the emulator always starts at `0x80000000`.
- `src/assembler.rs` declares it with `wasm-bindgen` and awaits the promise
  with `wasm-bindgen-futures`. The result is sent back to the UI through a
  channel that `GeckoApp::ui` polls each frame (egui has no async).
- The binutils files (~7 MB) are only fetched on the first `.s` drop, then
  cached by the browser.
- Everything runs locally in the student's browser: no code is uploaded
  anywhere.

### Portability fixes

- `std::time::Instant` panics on `wasm32-unknown-unknown`: use the
  [`web-time`](https://crates.io/crates/web-time) crate (same API, native
  passthrough).
- `std::thread` is not used anymore (single thread), `std::env::args` and
  `std::fs` only in the native `main.rs`.

## Licensing

- binutils is GPLv3. It is shipped as separate programs (aggregation), so
  gecko-sim's own license is unaffected, but the page must ship the GPLv3
  text and point to the corresponding source (binutils 2.46 release +
  racerxdl's `build.sh`). The racerxdl wrapper is MIT: keep its notice.
- gecko-sim needs a license of its own before going public (the vendored
  `lib-rv32` code is MIT, so MIT is the natural choice). Author's call.
- `mmio.ld` (the short course template linker script) is copied into `web/`
  so the build doesn't depend on the ignored `ressources/` folder. It
  contains no solution code.

## Error handling summary

| Failure | What the user sees |
|---|---|
| `.s` doesn't assemble / link | binutils output, verbatim |
| Dropped file is neither `.s` nor `.bin` | "Unsupported file: expected .s or .bin" |
| `.bin` too big for main RAM | load error message |
| CPU error at runtime | error + `pc`, board frozen on its last state |
| binutils files fail to load (offline, blocked) | "Could not load the assembler" + the underlying error |

## Testing

- **`Emulator` unit tests** (headless, like the existing ones): runs a tiny
  hand-encoded program; `run_for` stops advancing once stopped; a CPU error
  is captured with its `pc`; button presses reach the program.
- **Existing tests** (42) stay green: `Bus`, peripherals, CPU are unchanged.
- **Assembler pipeline, in CI**: a small public test program (written for
  the test, e.g. lights a few LEDs; **not** `gol.s`) is assembled both with
  the Ubuntu `riscv64-unknown-elf` GCC package and with `assemble.js` under
  Node; the two `.bin` files must be byte-identical.
- **Manual check before each release**: open the deployed page on at least
  two browsers, drop `gol.s` locally (never committed), click `b2` 6 times then `jr` 5 times
  (speed 5 used to freeze the board: signed `blt`/`bge` regression).

## Out of scope

- Other cs200 labs, other peripherals, RV32M/C.
- `.s` input in the native build.
- Debugging features (unchanged from the original design).
- Offline/PWA install.

## Impact on existing docs

- `CLAUDE.md`, "UI decision": amended — still `eframe`/`egui`, now also
  built for the web; threading model is single-threaded.
- `TODO.md` gets new steps for this work (pedagogical, no code), following
  the usual mode: the assistant writes the tests, the author writes the
  code.
