// Produce a dump to a file

use std::{fs::File, path::Path, io::Write, cell::Ref};
use std::ops::Deref;

use super::AddressingMode;
use super::decode::{LUT_6502, Instruction};
use super::mem::MemoryRead;
use super::{NESCpu, debug::disasm_6502};

pub struct TraceUnit {
    out_file: File,
}

impl TraceUnit {
    pub fn new(path: &Path) -> Self {
        Self {
            out_file: File::create(path).unwrap(),
        }
    }

    // Generates a single line in the text file, containing:
    // Address Mnemonic A XX Y P SP PPU: LINE, TICK Cycle
    // nestest will always report break lo as being 0... always
    // nestest will always report break hi as being 1... always

    pub fn dump(&mut self, cpu: &dyn Deref<Target = NESCpu>) {
        // Get instruction information
        let op = &cpu.memory.read(cpu.PC);
        let instr_opt = LUT_6502.get(&op);
        let instr: &Instruction;

        instr = instr_opt.unwrap_or_else(|| { panic!("Unknown instruction in trace unit: ${:0>4X}: {:0>2X}", cpu.PC, op) });

        let target_address: u16;
        if !matches!(instr.mode, AddressingMode::Implied | AddressingMode::Accumulator) {
            target_address = cpu.resolve_address(&instr.mode).0;
        } else {
            target_address = 0;
        }

        let line = format!(
            "{:0>4X}\t{}\t\tA:{:0>2X} X:{:0>2X} Y:{:0>2X} P:{:0>2X} SP:{:0>2X} CYC:{} RADDR:{:0>4X}\n",
            cpu.PC, disasm_6502(cpu.PC, &cpu.memory).0,
            cpu.A,
            cpu.X,
            cpu.Y,
            (cpu.status.bits() & 0xEF) | 0x20,
            cpu.SP,
            cpu.cycle,
            target_address,
        );

        self.out_file.write(line.as_bytes()).unwrap();
    }
}