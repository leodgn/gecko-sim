# Known issue: board freezes after starting the game and increasing speed

Status: **resolved** (2026-09-25). Date opened: 2026-09-25.

## Symptom

After a specific sequence of button presses, the board stops changing: the
7-segment step counter freezes, the LEDs stop evolving, and further button
presses have no visible effect.

## Exact reproduction steps

1. Launch the app with `cargo run -- ressources/gol.bin`.
2. Click `b2` **6 times** (sets the hundreds digit of the step count —
   without this, the game only runs for the default 1 step and returns to
   `INIT` immediately, per `GameOfLife.pdf` section 2.4.1).
3. Click the joystick's right button (`jr`) **5 times**: the first press
   starts the game from `INIT`, the following 4 presses raise the speed
   from 1 to 5 (section 2.4.3).
4. The board freezes as soon as the speed reaches 5.

## Root cause

A bug in the vendored RV32I core (`src/cpu/exec.rs`, inherited from
upstream `lib-rv32`): `blt` and `bge` compared their operands as
**unsigned** `u32` values instead of signed ones. (`bgeu` was also wrong: a
strict `>` instead of `>=`.)

`gol.s`'s `wait` procedure counts down like this:

```asm
  li s1, MAX_WAIT_TIME      # 0x30000
1:
  sub s1, s1, a3            # a3 = speed
  bgtz s1, 1b               # pseudo-instruction for: blt x0, s1, 1b
```

For speeds 1, 2, 3 and 4, `0x30000` is an exact multiple of the speed, so
`s1` lands exactly on 0 and the loop exits. For speed 5 (and 7, 9, 10...),
it isn't: `s1` goes from 3 straight to -2. `-2` as an unsigned `u32` is
`0xFFFFFFFE`, which compared as "greater than 0" — so the loop kept going
for about 2^32 / 5 ≈ 860 million more iterations (several minutes in a
debug build) instead of exiting. The program was stuck in `wait`, so
nothing on the board changed.

## How it was found

A debug copy of the app logged, once per second, the CPU thread's `pc`,
instruction count and the game variables (`CURR_STEP`, `SPEED`, ...), and
the UI thread's frame count. Reproducing with real clicks
(`hyprctl dispatch "hl.dsp.cursor.move({x=...,y=...})"` + `ydotool click
0xC0`) showed:

- The UI thread was **not** stuck: it kept rendering ~120 frames/s the whole
  time (a `gdb` backtrace also showed it inside normal egui frame code).
- The CPU thread kept executing ~2M instructions/s, but `CURR_STEP` stopped
  decreasing exactly when `SPEED` reached 5, and `pc` stayed on
  `0x800001b0`/`0x800001b4` — which disassembles to `sub s1, s1, a3` /
  `blt x0, s1, -4`, the `wait` loop above.

## Corrections to earlier assumptions

- The earlier hypothesis that this was lock contention between the UI and
  CPU threads was wrong. The high CPU usage seen on both threads is
  expected: the CPU thread never sleeps, and the UI thread renders a
  debug-build egui frame continuously (it already sits at high CPU at idle,
  before any button press).
- No "application not responding" dialog appeared during these repro
  runs; the window kept rendering (the same frame, because the emulated
  program was stuck).

## Fix

`src/cpu/exec.rs`: `blt`/`bge` now cast to `i32` before comparing, `bgeu`
uses `>=`. Regression tests in `src/cpu/mod.rs`
(`blt_and_bge_compare_as_signed`, `bgtz_is_not_taken_for_a_negative_register`,
`bltu_and_bgeu_compare_as_unsigned`, `bgeu_is_taken_when_operands_are_equal`).

Verified with the exact repro above, and further up to the maximum speed
(10 `jr` presses): the step counter keeps counting down at every speed, and
the `seed0` still lifes stay still.

## Related, not yet fixed (latent, not triggered by `gol.s` today)

- `auipc` computes `*pc + imm` without `wrapping_add`: panics in debug
  builds if the sum overflows (e.g. a negative offset from `0x8000_0000`).
- `jalr` doesn't clear bit 0 of the target address, as the spec requires.
