# gecko-sim

Fast Rust emulator for the **Gecko5** educational SoC from EPFL's cs200
course. Goal: replace the official `Vtb` simulator (cycle-accurate RTL,
extremely slow — several seconds per computation step on a modest program)
with something fast enough to iterate without waiting, for a personal
game-of-life project written in RISC-V assembly for the course.

It's also a **Rust learning project** for the author. Favor clarity and
idiomatic Rust over performance at all costs — the actual compute volume is
low (a few tens of thousands of RISC-V instructions per game step), Rust's
speed is comfortably enough for this use case regardless.

## Scope decision (already made, don't revisit without asking again)

**Don't rewrite the RV32I core by hand.** Reuse an existing Rust library for
instruction decode/execution, and build ourselves (that's the actual
substance of the project):
- loading the compiled binary (`.bin`, see `ressources/mmio.ld` for the base
  address);
- the memory bus with custom Gecko5 peripherals;
- the peripherals themselves (`LEDS`, `SEVEN_SEGS`, `BUTTONS`, `RANDOM`);
- a native GUI (see UI decision below), faithful to the `cs200` VS Code
  extension's behavior, without debugging features (no breakpoints/step —
  just "power on, it runs").

Full, locked-in design: `docs/superpowers/specs/2026-09-24-gecko-sim-design.md`.
Implementation roadmap (pedagogical, no code): `TODO.md`.

## UI decision (already made, don't revisit without asking again)

**Native Rust GUI with `egui`/`eframe`.** Deliberately ruled out: a
**Debug Adapter Protocol (DAP)** integration that would have let the existing
`cs200` VS Code extension attach directly to our emulator (verified: `Vtb`,
the official simulator, links `libcppdap` and implements a DAP server; the
extension is a generic DAP client that relays a custom `boardUpdate` event
and a custom `updateInput` request to a Svelte webview). Technically viable,
but the author prefers building their own UI rather than depending on that
protocol/on VS Code.

**Intended behavior**: faithful to the `cs200` extension (12×10×3-color LED
grid, 4 seven-segment displays, directional pad + 5 buttons, dip switches
for visual fidelity but not wired to any MMIO address), buttons driven by
mouse (not keyboard — that's what the reference extension does). No
debugging: the program starts running immediately on launch
(`PC = 0x80000000`) and runs continuously, like a real board being powered
on.

**Web build (amendment, 2026-09-27)**: the same `eframe` UI is also
compiled to WebAssembly and published on GitHub Pages, and both builds
accept a `.s` file (see
`docs/superpowers/specs/2026-09-25-web-distribution-design.md`). The
native threading model is unchanged; on the web, where threads aren't
available, the same `CpuRunner::run_batch` runs inside each UI frame.
Guiding rule for this work: change the existing code as little as
possible.

## Working mode (already decided, don't revisit without asking again)

**The author writes all the code themselves**, as a Rust learning exercise.
The assistant's role is to guide (explain the *how*, review, help unblock),
not to write the implementation on the author's behalf — except for one-off
explicit requests.

Concretely, step by step (follow the order in `TODO.md`):
1. The assistant writes the **tests** for the current step (and only the
   tests — not the implementation).
2. The author writes the code that makes those tests pass.
3. The assistant reviews, explains relevant Rust concepts as needed, then
   moves to the next step once tests are green.

**Language**: everything that goes into the project — code, comments, doc
comments, test names, assertion messages, commit messages — is in English.
No French in project artifacts. The assistant still talks to the author in
French in conversation; only what's written to files follows this rule.

### RV32I core: chosen path

**[`lib-rv32`](https://github.com/trmckay/lib-rv32)** (MIT, `trmckay`). Executes
instructions against any memory/register file that implements the
`lib_rv32_common::traits::{Memory, RegisterFile}` traits — exactly the hook
needed to plug custom peripherals into `Memory::load`/`Memory::store`.

**Verified via a spike** (see the design doc for the full write-up):
- The `Memory`/`RegisterFile` trait signatures match what was expected.
- The crate only implements pure RV32I in practice (no M/A/C), despite the
  README advertising `rv32imac`.
- **Confirmed bug**, found by actually running the code (not just reading
  it): in `exec.rs`, the branch distinguishing `add` from `sub` checks the
  wrong field (`func3` instead of `func7`), so `sub` always behaves like
  `add`. `gol.s` uses `sub` — this is a real correctness risk, not
  hypothetical.
- The repo has been dormant since August 2021 (single maintainer), but is
  small (~720 lines for `isa-sim`) and MIT-licensed.

**Decision**: vendor `lib-rv32-isa`'s source into the project (`src/cpu/`)
and patch the bug there, with the fix documented in a comment. No dependency
on a dormant repo, fully readable/auditable code, an assumed and documented
patch (nothing hidden).

## `ressources/` folder

- `GameOfLife.pdf` — full lab handout (25 pages). Source of truth in case of
  doubt about expected behavior.
- `hardware-spec.md` — condensed, verified summary of the memory map and
  peripherals (`LEDS`, `SEVEN_SEGS`, `BUTTONS`, `RANDOM`). **Start with this
  file**, only re-read the PDF if a detail is missing or seems inconsistent.
- `mmio.ld` — the course template's linker script (binary load address:
  `0x80000000`).
- `gol.s` — the author's game-of-life assembly implementation (functional,
  tested, but not guaranteed bug-free). Useful as:
  - a real test binary once assembled (`riscv64-unknown-elf-gcc
    -march=rv32i -mabi=ilp32 ...`, see the lab's Makefile, not copied here);
  - a concrete example of write sequences into the hardware registers
    (`LEDS`, `BUTTONS`, etc.), useful to verify the emulator interprets them
    as expected;
  - a source of known test patterns: `seed0` (at the bottom of the file)
    contains three classic still-life shapes (two 2×2 blocks, a beehive)
    that must stay **perfectly still** from one generation to the next — a
    good regression test for the game logic if the emulator is ever also
    used to validate `gol.s` itself, not just to run it fast.

## Useful history

This project started from a very concrete frustration: `update_gsa` (the
game-of-life's next-generation step) took 21 seconds per step in `Vtb`,
reduced to 2.43 seconds after assembly optimization — still judged too slow
to iterate comfortably. No further detail needed here; for questions about
*why* a given hardware constraint exists, `hardware-spec.md` and the PDF are
the sources to consult, not this history.
