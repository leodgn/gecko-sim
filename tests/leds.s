/* Test program for the assembler parity check (tests/assembler-parity.mjs):
   uses .equ, constant expressions, sections, li/la and a loop, and lights
   one LED per row. */
.equ LEDS, 0x50000000
.equ RED, 0x100
.equ ROWS, 10

.section .text
main:
    li   t0, LEDS
    la   t1, pattern
    li   t2, 0                  # row
loop:
    lw   t3, 0(t1)              # value for this row
    slli t4, t2, 4              # row field
    ori  t4, t4, 0xF            # all columns
    or   t4, t4, t3
    ori  t4, t4, RED
    sw   t4, 0(t0)
    addi t1, t1, 4
    addi t2, t2, 1
    li   t5, ROWS
    blt  t2, t5, loop
end:
    j    end

.section .data
pattern:
    .word 1 << 16, 1 << 17, 1 << 18, 1 << 19, 1 << 20
    .word 1 << 21, 1 << 22, 1 << 23, 1 << 24, 1 << 25
