# gecko-sim

A fast emulator for the Gecko5 board: an RV32I CPU, a 12×10 RGB LED
matrix, four 7-segment digits and ten push buttons. Give it a program, a
window opens, the program runs.

## Install

Requires [Rust](https://rustup.rs).

```sh
cargo install --git https://github.com/leodgn/gecko-sim --tag v0.1.0
```

## Usage

```sh
gecko-sim program.bin
```

Click the buttons with the mouse. Close the window to stop.

## Program conventions

- **RV32I only**: no `M`, `A`, `C` or `F` extensions.
- **Entry point**: execution starts at `0x80000000`, the first byte of the
  image. There's no `_start` lookup: put your first instruction first.
- **Input**: a raw binary image, loaded as is at `0x80000000`. With a
  RISC-V GNU toolchain:
  ```sh
  as -march=rv32i -mabi=ilp32 program.s -o program.o
  ld -m elf32lriscv -T mmio.ld program.o -o program.elf
  objcopy -O binary program.elf program.bin
  ```
  where `mmio.ld` is:
  ```ld
  OUTPUT_ARCH( "riscv" )
  ENTRY( _start)

  SECTIONS
  {
    . = 0x80000000;
    .text.init : { *(.text.init) }
    . = ALIGN(0x1000);
    .text : { *(.text) }
    . = ALIGN(0x1000);
    .data : { *(.data) }
    .bss : { *(.bss) }
   _end = .;
  }
  ```
  `ld`'s warning about a missing `_start` is expected.

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
  error is printed on the terminal and the board freezes on its last
  state.
- The dip switches are drawn but not connected to anything.
