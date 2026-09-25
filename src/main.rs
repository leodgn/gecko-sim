use std::env::args;
use std::fs::read;

use crate::bus::Bus;
use crate::cpu::exec_one;
use crate::regfile::RegisterFile;

mod bus;
mod cpu;
mod peripherals;
mod regfile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = args().nth(1).expect("usage: gecko-sim <path.bin>");
    let file = read(path)?;

    let mut bus = Bus::new();
    bus.store_byte(0x80000000, &file).expect("failed to load binary");

    let mut rf = RegisterFile::new();
    let mut pc: u32 = 0x80000000;

    loop {
        if let Err(e) = exec_one(&mut pc, &mut bus, &mut rf) {
            println!("{:?}", e);
            break;
        };
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::bus::Bus;
    use crate::cpu::exec_one;
    use crate::regfile::RegisterFile;

    /// This is the integration test for step 4: a hand-encoded program
    /// (no real gol.s .bin yet), loaded through `Bus::load_bytes`, then run
    /// through the exact same `exec_one` loop `main()` will use — just
    /// bounded to 3 steps here instead of "forever", since there's no
    /// natural way to know a hand-written program is "done".
    #[test]
    fn loads_and_runs_a_tiny_hand_encoded_program() {
        // addi x1, x0, 5
        // addi x2, x0, 7
        // add  x3, x1, x2      -> x3 should end up at 12
        let program: [u32; 3] = [0x00500093, 0x00700113, 0x002081b3];

        let mut bus = Bus::new();
        let mut bytes = Vec::new();
        for instruction in program {
            bytes.extend_from_slice(&instruction.to_le_bytes());
        }
        bus.store_byte(0x80000000, &bytes);

        let mut rf = RegisterFile::new();
        let mut pc: u32 = 0x80000000;
        for _ in 0..program.len() {
            exec_one(&mut pc, &mut bus, &mut rf).unwrap();
        }

        assert_eq!(rf.read(3), 12);
    }
}
