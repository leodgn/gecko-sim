# gecko-sim — Design

Date: 2026-09-24
Status: approved (see conversation history for the full brainstorm)

## Context and goal

Replace `Vtb` (the official RTL simulator for EPFL's cs200 course) with a
fast Rust emulator to iterate on `ressources/gol.s` (game of life in RISC-V
assembly). `Vtb` took 21s/step, down to 2.43s after optimization — still too
slow. The project is also used as a Rust learning exercise by the author:
**clarity and idiomatic style take priority over performance**, the actual
compute volume is low (tens of thousands of RISC-V instructions per step).

## Scope decisions (locked in by this document)

- RV32I core: **`lib-rv32`** (`trmckay/lib-rv32`, MIT), **vendored** into the
  repo (see the RV32I core section below), not rewritten by hand.
- Interface: **native Rust GUI (egui/eframe)**, no terminal, no DAP
  protocol. Explicit decision: although it's technically possible to make
  our emulator speak the Debug Adapter Protocol to attach the existing
  `cs200` VS Code extension directly to it (it launches `Vtb` as a DAP
  server and relays custom `boardUpdate`/`updateInput` events to a Svelte
  webview), the author prefers building their own UI rather than depending
  on that protocol.
- Intended behavior: **faithful to the `cs200` extension, visually and in
  its hardware logic**, but without debugging features (breakpoints, step)
  — like a real board that's powered on and immediately runs its program,
  continuously, until the app is closed.
- Working mode: **the author writes all the code themselves** (Rust
  learning exercise). This document fixes the architecture decisions;
  `TODO.md` is the pedagogical roadmap, with no code.

## RV32I core: `lib-rv32` (vendored, with a fix)

Verification spike performed (see conversation history):

- The `Memory`/`RegisterFile` traits (`lib-rv32-isa` crate, `traits`
  module) match exactly the intended hook: `fetch`,
  `read_word/half_word/byte`, `write_word/half_word/byte` for `Memory`;
  `read(num)`/`write(num, data)` for `RegisterFile`.
- The execution core is a free function `exec_one(pc: &mut u32, mem: &mut
  M, rf: &mut R) -> Result<(), RiscvError>` — one call per instruction, no
  hidden state. It only decodes pure RV32I (no M/A/C), despite what the
  repo's README advertises.
- **Two stacked bugs confirmed by actually running the code** (not just
  reading it), both in `isa-sim/src/exec.rs`'s `add`/`sub` branch (same
  `func3`, normally distinguished by `func7`):
  1. It mistakenly matches on `decode_func3!(ir)` instead of
     `decode_func7!(ir)`, so the `FUNC7_SUB` arm is unreachable — every
     add/sub silently falls through to the `add` arm.
  2. Even the `FUNC7_SUB` arm's body computed `l.wrapping_add(r)` instead
     of `l.wrapping_sub(r)` — fixing bug 1 alone would not have been
     enough, `sub` still wouldn't have subtracted.
  Consequence: **`sub` executes as `add`**. Verified by hand-encoding
  `sub x5, x6, x7` with `x6=10, x7=3`: result `13` (10+3) instead of `7`
  (10-3). `gol.s` uses `sub` (line 232, speed timer countdown) — a real
  correctness risk, not hypothetical.
- The repo has been dormant since August 25, 2021, single maintainer, MIT,
  ~720 lines for all of `isa-sim`. Published on crates.io (`lib-rv32-isa`
  0.2.0, `lib-rv32-common` 0.2.0).

**Decision**: vendor `lib-rv32-isa`'s code (~720 lines) as an internal
project module (`src/cpu/`), fix the bug with a comment explaining it. No
dependency on a dormant repo, fully readable/auditable code, a documented
and assumed fix (no hidden patched version to ship).

## Architecture

A single binary crate (no workspace, the project is too small to justify
one).

```
src/
  main.rs          — entry point: loads the .bin, spawns the CPU thread, runs eframe
  cpu/              — vendored isa-sim (RV32I decode/exec) + the documented sub/add patch
  regfile.rs        — implements RegisterFile ([u32; 32], x0 hardwired to 0)
  bus.rs            — implements Memory: dispatches by address range to RAM or peripherals
  peripherals/
    leds.rs         — decodes write commands, maintains the 10×12×3 framebuffer
    seven_segs.rs
    buttons.rs      — bits + "falling edge" logic + clear-on-any-write
    random.rs       — xorshift32, fixed seed (same sequence on every run)
  loader.rs         — reads the .bin, places it at 0x80000000
  ui.rs             — the eframe app: draws LEDs/7-seg/buttons/joystick/dip switches, captures mouse clicks
```

### Execution flow

At startup: load the binary, spawn a dedicated thread that loops
`exec_one(pc, bus, regfile)` continuously (as fast as possible), while the
main thread runs `eframe`. Shared state (LED framebuffer, 7-seg state,
`BUTTONS` register) behind an `Arc<Mutex<...>>`: the CPU thread writes, the
UI thread reads each frame (~60 Hz) and writes user input. No fine-grained
synchronization needed (scale: tens of thousands of instructions per game
step) — clarity over performance.

### Entry point

The `.bin` is already `objcopy`'d (raw format, no symbol table). No `_start`
ambiguity to resolve (contrary to what `hardware-spec.md` suggested):
**the PC always starts at `0x80000000`** on "power on".

### Memory (`bus.rs`)

Dispatch by address range (see `ressources/hardware-spec.md`, Table 2):

| Address | Behavior |
|---|---|
| `0x40000000` | `RANDOM` (peripheral) |
| `0x50000000` | `LEDS` (peripheral, write-only) |
| `0x60000000` | `SEVEN_SEGS` (peripheral) |
| `0x70000004` | `BUTTONS` (peripheral) |
| `0x80000000` and up | flat RAM (code/data/stack) |
| `0x90001000`–`0x90001300` | flat RAM (game state, GSA, custom variables) |
| elsewhere | memory access error |

Two separate RAM regions (not a single giant `Vec` spanning the whole 32-bit
address space): one for `0x80000000+` (code/data/stack), one for
`0x90001000..0x90001300` (game state). Sizes chosen reasonably generously.

### Peripherals

- **`LEDS` (0x50000000)** — write-only. Every write is a command
  (row/col/color/value, see `hardware-spec.md`), not stored state. Internal
  framebuffer `[[u8; 12]; 10]` per color (r/g/b). Reads always return 0.
- **`SEVEN_SEGS` (0x60000000)** — 4 bytes packed into a word, normal
  read/write (read behavior undocumented in the PDF → normal RAM by
  default).
- **`BUTTONS` (0x70000004)** — 10 physical bits, two groups of 5 in the UI
  (as in the `cs200` extension):
  - directional pad: JT/JB/JL/JR/JC (bits 4,3,2,1,0)
  - button row: BUTTON_0/BUTTON_1/BUTTON_2 (bits 6,5,7) + 2 bits unnamed in
    the template (8,9)
  Semantics: a mouse click sets the bit to 1 on the falling edge
  (released→pressed); it stays 1 until the CPU writes anything to the
  register (clears everything at once). **Mouse interaction**
  (mousedown/mouseup), not keyboard — faithful to the `cs200` extension
  (Svelte components `PushButton`/`JoyStick` observed, all mouse-driven).
- **`RANDOM` (0x40000000)** — xorshift32 (or LCG) with a **fixed, hardcoded
  seed**. Confirmed in `GameOfLife.pdf` section 3.4.1: "It is therefore
  expected that the random number generator will return the same sequence
  of numbers each time the program is run." → same sequence on every run of
  our emulator. No attempt to reproduce `Vtb` bit-for-bit (impossible
  without its RTL source).
- **Dip switches** — displayed in the UI for visual fidelity with the
  `cs200` extension (Svelte component `dipSwitches`), but **not wired to
  any MMIO address**: neither the memory map nor `gol.s` use them.
  Interactive widget with no effect on emulation, documented as such in a
  comment.

### Error handling

A CPU error (invalid opcode, out-of-map memory access) stops the CPU thread
and surfaces the error visibly in the UI — no silent crash, useful since the
tool exists specifically to debug `gol.s`.

### Tests

- Unit tests per peripheral: `LEDS` decoding (the 4 row/column selection
  cases), `BUTTONS` (falling edge + clear-on-any-write), `RANDOM`
  (determinism: two fresh instances give the same sequence).
- Regression test based on `gol.s`'s `seed0` (mentioned in `CLAUDE.md`):
  load compiled `gol.s`, run one generation, check via the LED framebuffer
  that the 3 still-life shapes (2 2×2 blocks + 1 beehive) are identical
  before/after. Feasible headless (the framebuffer is a plain data
  structure, testable without launching `eframe`).
- Test for the `sub`/`add` fix in the vendored `cpu/` module.

## Rejected / out of scope

- **DAP protocol + reusing the `cs200` extension**: technically viable
  (verified: `Vtb` links `libcppdap`, the extension is a generic DAP client
  that listens for a custom `boardUpdate` event and sends a custom
  `updateInput` request), but explicitly ruled out by the author in favor
  of a clean Rust UI.
- **Bit-exact reproduction of `Vtb` for `RANDOM`**: impossible without the
  hardware generator's RTL source.
- **Hand-written RV32I core**: ruled out, `lib-rv32` (vendored + patched)
  is more than enough.
- **Debugging (breakpoints, step)**: out of scope, contrary to the intended
  goal (speed, "real board being powered on" behavior).
