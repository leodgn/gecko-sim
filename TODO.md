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

- [x] `src/bus.rs`: `Bus` with two flat `Vec<u8>` regions (main:
      `0x80000000`+, game state: `0x90001000..0x90001300`), `locate()`
      translates an address to (region, offset), `impl Memory for Bus`.
- [x] `read_bytes`/`write_bytes` factor out the shared little-endian
      byte-combining logic between the word/half-word variants.
- [x] All 5 bus tests pass (17 total).

*Rust concepts: `Vec<u8>`, indexing, error handling with `Result` for
out-of-range accesses.*

## 4. Binary loader

- [x] `Bus::store_byte(&mut self, addr: u32, bytes: &[u8]) -> Result<(),
      RiscvError>` — writes a whole byte slice starting at `addr`, one byte
      at a time via `write_byte`, propagating errors with `?`. (Named
      `store_byte`, not `load_bytes` — matches the CPU's own STORE/LOAD
      vocabulary already used in `exec.rs`: writing to memory is a
      "store", reading a register out to memory would be a "load" from
      the CPU's point of view. Good catch.)
- [x] `main()` reads a `.bin` path from the first CLI argument
      (`std::env::args().nth(1)`), loads it via `std::fs::read` + `?`
      (note: `main`'s signature had to become
      `fn main() -> Result<(), Box<dyn std::error::Error>>` for `?` to be
      usable at all — it needs a `Result`-returning function to propagate
      into), then `store_byte`s it into a fresh `Bus` at `0x80000000`. No
      real `gol.s` `.bin` tested yet (no RISC-V toolchain confirmed
      installed) — validated instead via the hand-encoded program test
      below.
- [x] Minimal execution loop in `main()`: `pc = 0x80000000`, `loop { if let
      Err(e) = exec_one(&mut pc, &mut bus, &mut rf) { println!("{:?}", e);
      break; } }`.
- [x] Test written in `src/main.rs` (or wherever the loop ends up):
      hand-encode a handful of RV32I instructions (reuse
      `cpu::instructions` constants or hand-encode like in the sub/add
      spike), load them via `load_bytes`, run the loop, check the
      registers hold the expected values at the end.

*Rust concepts: `std::fs`, `io::Result`, controlled infinite loops,
`std::process::exit` or `panic!` for a clean stop on error.*

## 5. Peripherals (one at a time, with a unit test each time)

Do them in this order (simplest to most useful for fast validation):

- [x] **`RANDOM`** (`0x40000000`): xorshift32 with a fixed, hardcoded seed
      (`src/peripherals/random.rs`). Both tests pass (20 total).
- [x] **`BUTTONS`** (`0x70000004`, `src/peripherals/buttons.rs`): "falling
      edge" logic (`press` sets a bit, stays until `clear` resets the whole
      register). 5 tests pass.
- [x] **`SEVEN_SEGS`** (`0x60000000`, `src/peripherals/seven_segs.rs`):
      plain read/write of a `u32`. 2 tests pass.
- [x] **`LEDS`** (`0x50000000`, `src/peripherals/leds.rs`): the most
      complex of the 4 — decodes the 4 row/column selection cases from
      `hardware-spec.md`. Storage ended up as `[[bool; 12]; 10]` per color
      rather than the bitmask originally sketched in the design doc —
      simpler to decode into, one boolean per LED; a bitmask conversion
      (e.g. to match the `cs200` extension's `LedArray_t` shape) can happen
      later, isolated in the UI code, if needed. 6 tests pass (33 total).
- [x] Wired the 4 peripherals into `Bus::read_word`/`write_word` (byte/
      half-word access stays RAM-only — `gol.s` only ever touches
      peripherals via `lw`/`sw`, full words).
      Hit a real design tension: `read_word` only gets `&self` (per the
      vendored `Memory` trait), but reading `RANDOM` has a genuine side
      effect (it advances the PRNG). Two ways to resolve it were discussed
      — changing the vendored trait's read methods to `&mut self` (free,
      since `exec_one` already only ever holds `&mut M`), vs. wrapping
      `random: RefCell<Random>` for interior mutability. Went with
      `RefCell` (`self.random.borrow_mut().next()`) to move faster; the
      trait-redesign alternative is still on the table if this ever feels
      wrong later.
      `Bus::press_button` and `Bus::leds()` added as the UI-facing entry
      points (separate from the CPU-facing `write_word`/`read_word`
      dispatch, which only `exec_one` calls). 37 tests pass.

*Rust concepts: `match` on ranges/bits, bitwise operations (`&`, `|`, `<<`,
`>>`), unit tests (`#[test]`, `assert_eq!`).*

## 6. Multithreading: CPU running continuously + shared state

- [x] Design choice: share the **whole `Bus`** behind one
      `Arc<Mutex<Bus>>`, rather than a separate `Arc<Mutex<...>>` per piece
      of state (LEDs/7-seg/BUTTONS) as originally sketched — one lock,
      simpler to reason about, matches "clarity over perf".
- [x] `main()`: `bus` is loaded once (`store_byte`) before spawning; `rf`
      and `pc` live *inside* the spawned closure (never shared, only the
      CPU thread ever needs them); the loop takes the lock once per
      instruction (`&mut *cpu_bus.lock().unwrap()`), never across a whole
      iteration. `main()` itself just `handle.join()`s at the end for now
      (no UI thread yet — that's step 7).
- [x] Verified against the real `gol.s` binary: runs continuously for
      several seconds without crashing (stuck in `gol.s`'s own `INIT`
      state loop waiting for button input, which doesn't exist yet — but
      the whole load → CPU → bus → peripherals → dedicated-thread chain
      works end to end). 38 tests pass.

*Key Rust concepts for this project: `Arc`, `Mutex`, `std::thread`, `move`
closures. Probably the most "new" part if you're coming from Scala — no
actors here, just classic locked shared state.*

## 7. UI (`egui`/`eframe`)

- [x] Skeleton `eframe` app that loops and reads the shared state each
      frame (no need for special VSync handling, `egui` takes care of it).
- [x] Draw the 12×10 color LED grid (colored rectangles, see
      `LedArray.svelte` from the `cs200` extension for the exact visual
      reference if you want to match the look).
- [x] Draw the 4 seven-segment displays (`font_data` table in `gol.s` to
      interpret the segment patterns).
- [x] Draw the directional pad (5 buttons) + the second row of 5 buttons +
      the dip switches (visual widget only, not wired). Buttons clickable
      with the mouse (mousedown → bit to 1, mouseup/leave → nothing special
      on the emulator side, the bit stays until a CPU clear).
- [x] "Load a .bin" control (file picker or CLI argument, your choice) that
      (re)starts emulation.

*Rust concepts: `egui::Context`, immediate mode (drawing code runs every
frame, no retained widget tree like a classic UI — quite different from
what you may have seen elsewhere).*

## 8. Regression test with `gol.s`

- [x] Load `gol.s` compiled with `seed0`, run one generation (headless, no
      `eframe` — just the CPU loop + the framebuffer), check via the LED
      framebuffer that the 3 still-life shapes (2 2×2 blocks + 1 beehive)
      are identical before/after.

## 9. Polish (optional, once everything works)

- [x] Clean error display in the UI if the CPU crashes (invalid opcode,
      out-of-map memory access) instead of a process crash.
- [x] Display speed: if 60 Hz rendering struggles to keep up with a CPU
      running at full speed, consider a throttle or simply showing the
      "latest visible state" without blocking the CPU thread.

## 10. `.s` input + web distribution (`v0.2.0`)

Behavior-preserving refactors (like extracting `CpuRunner`) land on
`master`; the new features go on a `web` branch.

Design: `docs/superpowers/specs/2026-09-25-web-distribution-design.md`.
`v0.1.0` (tag) is the native app as it was before this section. Keep the
existing code as it is: changes here are moves or additions.

- [ ] **Extract `CpuRunner`** (`src/runner.rs`): move the CPU-thread
      closure's state and one iteration of its outer `loop` into
      `CpuRunner::new` / `run_batch`. `main.rs` spawns the same thread,
      which now just calls `run_batch` in a loop. Native behavior must not
      change. Pure move: keep `std::time` for now.
- [ ] **Native `.s` input** (`src/assembler/native.rs`): find a local
      RISC-V toolchain, run `as` → `ld` → `objcopy` in a temporary
      directory with the embedded `assets/mmio.ld`. `main.rs` picks by
      extension.
- [ ] **Web skeleton**: `cfg` split (`main.rs` native-only, `src/web.rs`
      wasm-only), Trunk + `index.html` from `eframe_template`,
      `std::time::Instant` → `web_time::Instant` (the std one panics in
      the browser). The page
      shows the board, off, with the "drop your file" message.
- [ ] **Web `.bin` drop**: dropping a `.bin` creates a `CpuRunner` and the
      board runs; `run_batch` inside each frame. CPU errors shown in a
      panel.
- [ ] **Web `.s` drop**: vendor the binutils wasm files, write
      `web/assemble.js`, bridge it from `src/assembler/web.rs` with
      `wasm-bindgen`, poll the result through a channel. Assembler errors
      shown verbatim.
- [ ] **CI + Pages**: GitHub Action running the tests, the native-vs-web
      assembler parity check, the Trunk build and the Pages deploy.
- [ ] **Release**: license (MIT?), README (web link + `cargo install`
      for both tags), binutils GPLv3 notice on the page, then
      `git merge --no-ff web` into `master` and tag `v0.2.0`.

*Rust concepts: moving code into a struct with `&mut self` methods,
`#[cfg(target_arch = ...)]`, `std::process::Command`, `wasm-bindgen`,
futures without an async runtime (`spawn_local` + a channel).*
