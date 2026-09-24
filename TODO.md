# Roadmap — gecko-sim

Pedagogical, not an execution plan: **you write the code**. Check items off
as you go. Ask for help at any step if you get stuck — the idea is that I
explain the *how* (Rust concepts, pitfalls, API) without writing the
implementation for you.

Full design: `docs/superpowers/specs/2026-09-24-gecko-sim-design.md`.
Hardware reference: `ressources/hardware-spec.md`.

## 0. Setup

- [x] `cargo init` (binary crate, no workspace).
- [x] Add `eframe`/`egui` as a dependency (just to check it compiles and
      shows an empty window — no logic yet).
      `cargo add eframe`: done. Run `cargo run --example eframe_check`
      (throwaway file already written, `examples/eframe_check.rs`) and
      check that the window opens. Once confirmed, delete
      `examples/eframe_check.rs`.
- [x] Fetch `lib-rv32-isa`'s sources to prepare vendoring for step 2 —
      cloned directly into `lib-rv32/` at the project root (added to
      `.gitignore`, it's just a temporary working directory).

*Rust concepts: structure of a binary crate, `Cargo.toml`, `cargo run`.*

## 1. Register file

- [x] `src/regfile.rs`: `RegisterFile` struct (home-grown API for now, not
      yet lib-rv32's trait — that comes in step 2), storage `[u32; 32]`,
      `x0` always 0 (guaranteed by ignoring writes to `x0` in `write`,
      rather than special-casing `read`).
- [x] 4 tests pass (`cargo test`).

*Rust concepts: `trait` + `impl` for a type, fixed-size arrays `[T; N]`,
`Result`.*

## 2. Vendor `isa-sim` + fix the sub/add bug

- [x] Copy the files from `lib-rv32-isa/src/` (decode.rs, exec.rs,
      traits.rs, error.rs — plus `bits.rs`/`constants.rs`/`instructions.rs`
      from `lib-rv32-common/src/`) into a `src/cpu/` module of your
      project.
- [x] Adjust imports. Two subtleties found along the way, worth
      remembering: (1) `#[macro_export]` macros (like `bit_concat!`,
      `decode_i_imm!`, ...) always land at the actual crate root, never
      inside the module where they're textually defined — so those
      specific imports need plain `crate::`, not `crate::cpu::`, unlike
      every other item; (2) `exec.rs` uses the `log` crate (`cargo add
      log`), not vendored, just a normal dependency.
- [x] Fixed the add/sub bug in `exec_one` — turned out to be **two bugs
      stacked**, not one: the branch matched on `decode_func3!(ir)`
      instead of `decode_func7!(ir)` (making the `sub` arm unreachable),
      *and* the `sub` arm itself computed `l.wrapping_add(r)` instead of
      `l.wrapping_sub(r)` (so fixing only the match wouldn't have been
      enough). Both fixed, with a comment in `exec.rs` explaining why.
- [x] `impl cpu::traits::RegisterFile for RegisterFile` added in
      `src/regfile.rs`, delegating to the existing `read`/`write` methods.
- [x] All 12 tests pass (`cargo test`), including the sub/add regression
      test.

*Rust concepts: modules (`mod`), visibility (`pub`), macros (no need to
understand `macro_rules!` in depth, just how to use them).*

## 3. Memory bus (flat RAM, no peripherals yet)

- [ ] Write a `Bus` type implementing lib-rv32's `Memory` trait (`fetch`,
      `read_word/half_word/byte`, `write_word/half_word/byte`). For now:
      just two flat RAM regions (see the memory map in the spec) — no
      peripheral dispatch yet.
- [ ] Watch out for the address → index translation into the `Vec`/array
      (address `0x80000000` must not be the literal index 0 of a
      multi-gigabyte `Vec` — compute a per-region offset).
- [ ] Test: write a word at an address, read it back, check equality. Also
      test `read_byte`/`read_half_word` on a word you just wrote (mind
      endianness — RISC-V is little-endian).

*Rust concepts: `Vec<u8>`, indexing, error handling with `Result` for
out-of-range accesses.*

## 4. Binary loader

- [ ] Read a `.bin` file (`std::fs::read`) and copy it into the `Bus`'s RAM
      starting at `0x80000000`.
- [ ] Minimal execution loop: `pc = 0x80000000`, a loop calling
      `exec_one(&mut pc, &mut bus, &mut regfile)` inside `loop {}`, print
      the error and stop if `exec_one` returns `Err`.
- [ ] Manual test: write a tiny RISC-V assembly program (a few RV32I
      instructions), assemble it into a `.bin` (`riscv64-unknown-elf-gcc
      -march=rv32i -mabi=ilp32 ...` + `objcopy`, see the course Makefile),
      load it, check via logs that the registers hold the expected values
      at the end.

*Rust concepts: `std::fs`, `io::Result`, controlled infinite loops,
`std::process::exit` or `panic!` for a clean stop on error.*

## 5. Peripherals (one at a time, with a unit test each time)

Do them in this order (simplest to most useful for fast validation):

- [ ] **`RANDOM`** (`0x40000000`): xorshift32 with a fixed, hardcoded seed.
      Test: two fresh instances produce the same sequence of reads.
- [ ] **`BUTTONS`** (`0x70000004`): "falling edge" logic (a click sets the
      bit to 1, it stays 1) + "any write clears the whole register". Test:
      simulate a click, read the register, simulate a CPU write, check
      everything is 0.
- [ ] **`SEVEN_SEGS`** (`0x60000000`): normal read/write of a 4-byte word.
      Trivial test (write/read back).
- [ ] **`LEDS`** (`0x50000000`): the most complex of the 4 — decode the 4
      row/column selection cases from `hardware-spec.md` and update a
      `[[u8; 12]; 10]` framebuffer per color. Write-only, reads always
      return 0. Test each selection case separately (all rows + all
      columns, one column, one row, a single LED).
- [ ] Wire the 4 peripherals into `Bus::read_*`/`write_*` by address range
      (replace the bus's TODO with real dispatch).

*Rust concepts: `match` on ranges/bits, bitwise operations (`&`, `|`, `<<`,
`>>`), unit tests (`#[test]`, `assert_eq!`).*

## 6. Multithreading: CPU running continuously + shared state

- [ ] Run the execution loop on a dedicated thread (`std::thread::spawn`).
- [ ] Share the LED framebuffer, seven-seg state, and `BUTTONS` register
      between the CPU thread and the future UI thread via
      `Arc<Mutex<...>>`.
- [ ] Check it compiles and runs without deadlocking (the CPU thread must
      never hold the lock longer than needed — take the lock, read/write,
      release, no lock held across a whole loop iteration).

*Key Rust concepts for this project: `Arc`, `Mutex`, `std::thread`, `move`
closures. Probably the most "new" part if you're coming from Scala — no
actors here, just classic locked shared state.*

## 7. UI (`egui`/`eframe`)

- [ ] Skeleton `eframe` app that loops and reads the shared state each
      frame (no need for special VSync handling, `egui` takes care of it).
- [ ] Draw the 12×10 color LED grid (colored rectangles, see
      `LedArray.svelte` from the `cs200` extension for the exact visual
      reference if you want to match the look).
- [ ] Draw the 4 seven-segment displays (`font_data` table in `gol.s` to
      interpret the segment patterns).
- [ ] Draw the directional pad (5 buttons) + the second row of 5 buttons +
      the dip switches (visual widget only, not wired). Buttons clickable
      with the mouse (mousedown → bit to 1, mouseup/leave → nothing special
      on the emulator side, the bit stays until a CPU clear).
- [ ] "Load a .bin" control (file picker or CLI argument, your choice) that
      (re)starts emulation.

*Rust concepts: `egui::Context`, immediate mode (drawing code runs every
frame, no retained widget tree like a classic UI — quite different from
what you may have seen elsewhere).*

## 8. Regression test with `gol.s`

- [ ] Load `gol.s` compiled with `seed0`, run one generation (headless, no
      `eframe` — just the CPU loop + the framebuffer), check via the LED
      framebuffer that the 3 still-life shapes (2 2×2 blocks + 1 beehive)
      are identical before/after.

## 9. Polish (optional, once everything works)

- [ ] Clean error display in the UI if the CPU crashes (invalid opcode,
      out-of-map memory access) instead of a process crash.
- [ ] Display speed: if 60 Hz rendering struggles to keep up with a CPU
      running at full speed, consider a throttle or simply showing the
      "latest visible state" without blocking the CPU thread.
