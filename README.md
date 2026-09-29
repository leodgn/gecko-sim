# gecko-sim

A fast emulator for the Gecko5 board: an RV32I CPU, a 12×10 RGB LED
matrix, four 7-segment digits and ten push buttons. Give it a program, it
runs.

## In the browser

**<https://leodgn.github.io/gecko-sim/>**: nothing to install. Drop a `.s`
or `.bin` file on the page. `.s` files are assembled in the browser with
GNU binutils compiled to WebAssembly; nothing is uploaded. Dropping another
file restarts the board.

## Native

Requires [Rust](https://rustup.rs).

```sh
cargo install --git https://github.com/leodgn/gecko-sim --tag v0.3.0
```

To run `.s` files, a RISC-V GNU toolchain must also be on your `PATH`.
The first one found among these prefixes is used: `riscv64-unknown-elf-`,
`riscv64-elf-`, `riscv64-linux-gnu-`, `riscv32-unknown-elf-`,
`riscv-none-elf-`, `riscv-none-embed-`. Running `.bin` files needs
nothing else.

### Usage

```sh
gecko-sim program.s     # assembled and linked, then run
gecko-sim program.bin   # raw binary image, run as is
```

Assembler and linker errors are printed as is, and the window doesn't
open. Click the buttons with the mouse. Close the window to stop.

## Program conventions

- **RV32I only**: no `M`, `A`, `C` or `F` extensions.
- **Entry point**: execution starts at `0x80000000`, the first byte of the
  image. There's no `_start` lookup: put your first instruction first.
- **Linking** (`.s` files): the equivalent of
  ```sh
  as -march=rv32i -mabi=ilp32 program.s -o program.o
  ld -m elf32lriscv -T assets/mmio.ld program.o -o program.elf
  objcopy -O binary program.elf program.bin
  ```
  [`assets/mmio.ld`](assets/mmio.ld) places `.text.init` (if any) at
  `0x80000000`, then `.text` and `.data`, each on the next 4 KiB boundary.
  `ld`'s warning about a missing `_start` is expected.
- **`.bin` files** are loaded as is at `0x80000000`.

### Memory map

| Address                   | Device       | Access                                  |
|---------------------------|--------------|-----------------------------------------|
| `0x40000000`              | `RANDOM`     | read: next pseudo-random word           |
| `0x50000000`              | `LEDS`       | write: LED command (see below); reads 0 |
| `0x60000000`              | `SEVEN_SEGS` | read/write: one segment byte per digit  |
| `0x70000004`              | `BUTTONS`    | read: pressed bits; any write clears    |
| `0x80000000`–`0x800FFFFF` | Main RAM     | code, data, stack (1 MiB)               |
| `0x90001000`–`0x900012FF` | RAM          | general purpose                         |

Peripherals only accept word accesses. Any other address is an error that
stops the CPU.

### Peripherals

**`LEDS`**: each write is a command, not a state.

| Bits  | Field                                              |
|-------|----------------------------------------------------|
| 31–16 | `value`                                            |
| 10    | blue selected                                      |
| 9     | green selected                                     |
| 8     | red selected                                       |
| 7–4   | row (0–9, or `0xF` for all rows)                   |
| 3–0   | column (0–11, or `0xF` for all columns)            |

For the selected colors: one LED takes `value[0]`; a whole row takes
`value[11:0]` (bit 0 = leftmost); a whole column takes `value[9:0]`
(bit 0 = top); all LEDs take `value[0]`.

**`SEVEN_SEGS`**: byte 0 drives the rightmost digit, byte 3 the leftmost.
Each byte is a segment pattern (`0x3F` shows `0`, `0x06` shows `1`, ...),
not a number.

**`BUTTONS`**: bit *i* goes to 1 when button *i* is pressed and stays set
until the program writes to the register, which clears all bits at once.
Bits 0–4 are the directional pad (center, right, left, bottom, top), bits
5–9 the other buttons.

**`RANDOM`**: each read returns the next value of a pseudo-random sequence.

## Limitations

- No debugger: no breakpoints, no stepping.
- If the CPU hits an error (invalid instruction, bad memory access), the
  error is shown (on the page, or on the terminal for the native build)
  and the board freezes on its last state.
- The dip switches are drawn but not connected to anything.

## License

gecko-sim is under the MIT license: see [`LICENSE`](LICENSE).

## Credits

The RV32I core in [`src/cpu/`](src/cpu) is adapted from
[lib-rv32](https://github.com/trmckay/lib-rv32), Copyright (c) 2021
Trevor McKay, under the MIT license (see [`src/cpu/LICENSE`](src/cpu/LICENSE)).
It was modified to fix `sub`, `sll`, `blt`/`bge` and `bgeu`.

The web version's assembler is [GNU binutils](https://www.gnu.org/software/binutils/)
(`as`, `ld`, `objcopy`), compiled to WebAssembly by
[riscv-online-asm](https://github.com/racerxdl/riscv-online-asm), under the
GNU GPL v3: see [`web/binutils/`](web/binutils) for its licenses and source.
