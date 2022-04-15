use std::cell::Ref;
use std::ops::Deref;

use crate::cpu::decode::{LUT_6502, Instruction};

/// Provide the facilities necessary for the nes-platform
/// crate to generate a disasm view of the current NES PRG.

use super::{AddressingMode, mem::CPUMemory, NESCpu};

pub fn cpu_dump<'a>(cpu: impl Deref<Target = NESCpu<'a>>) -> String {
    let mut dump: String = String::new();
    let items_on_stack = 0xFF - cpu.SP;

    dump.push_str(format!("CORE DUMPED @ ${:X}\n", cpu.PC).as_str());
    dump.push_str(format!("\tA: {:X}, X: {:X}, Y: {:X}, PC: ${:X}\n", cpu.A, cpu.X, cpu.Y, cpu.PC).as_str());
    if cpu.last_legal_instruction.is_some() {
        dump.push_str(format!("\tPrevious: ${:X}: {}\n", 
            cpu.last_legal_instruction.unwrap(),
            disasm_6502(cpu.last_legal_instruction.unwrap(), &cpu.memory).0.as_str()).as_str());
    }
    dump.push_str(format!("Stack (descending - {} items)\n", items_on_stack).as_str());
    for saddr in ((cpu.SP as u16+0x0101)..=0x01FFu16).rev() {
        dump.push_str(format!("${:X}: {:0>2X}\n", saddr, cpu.memory.observe(saddr)).as_str());
    }

    dump
}

// Returns the string of disassembly, as well as the address delta to the next
// instruction.
pub fn disasm_6502(instruction_addr: u16, mem: &CPUMemory) -> (String, u16) {
    use AddressingMode::*;

    let opcode = &mem.observe(instruction_addr);
    let instr_opt = LUT_6502.get(opcode);
    let instr: &Instruction;
    let operand: u16;

    if instr_opt.is_none() {
        return (format!("Unknown disassembly for opcode {:X}", opcode), 0);
    }

    instr = instr_opt.unwrap();

    match instr.mode {
        AddressingMode::ZeroPage |
        AddressingMode::ZeroPageX |
        AddressingMode::ZeroPageY |
        AddressingMode::IndirectIndexed |
        AddressingMode::IndexedIndirect |
        AddressingMode::Immediate |
        AddressingMode::Relative => {
            operand = mem.observe(instruction_addr + 1) as u16;
        },
        AddressingMode::Absolute |
        AddressingMode::AbsoluteX |
        AddressingMode::AbsoluteY |
        AddressingMode::Indirect => {
            operand = mem.observe_16(instruction_addr + 1);
        },
        _ => { operand = 0xDEAD; }
    }
    let disasm: (String, u16);

    match instr.mode {
        Accumulator => {
            disasm = (format!("{0}", instr.mnemonic), 1);
        }
        Implied => {
            disasm = (format!("{0}", instr.mnemonic), 1);
        }
        Immediate => {
            disasm = (format!("{0} #${1:X}", instr.mnemonic, operand as u8), 2);
        }
        Absolute => {
            disasm = (format!("{0} ${1:X}", instr.mnemonic, operand), 3);
        }
        ZeroPage => {
            disasm = (format!("{0} ${1:X}", instr.mnemonic, operand as u8), 2);
        }
        Relative => {
            if (operand as u8) & 0b10000000 > 0 {
                disasm = (format!("{0} ${1:X} (-${2:X})", instr.mnemonic, operand as u8,
                    !(operand as u8) + 1), 2)
            } else {
                disasm = (format!("{0} ${1:X}", instr.mnemonic, operand as u8), 2);
            }
        }
        ZeroPageX => {
            disasm = (format!("{0} ${1:X},X", instr.mnemonic, operand as u8), 2);
        }
        ZeroPageY => {
            disasm = (format!("{0} ${1:X},Y", instr.mnemonic, operand as u8), 2);
        }
        Indirect => {
            disasm = (format!("{0} (${1:X})", instr.mnemonic, operand), 3);
        }
        AbsoluteX => {
            disasm = (format!("{0} ${1:X},X", instr.mnemonic, operand), 3);
        }
        AbsoluteY => {
            disasm = (format!("{0} ${1:X},Y", instr.mnemonic, operand), 3);
        }
        IndexedIndirect => {
            disasm = (format!("{0} (${1:X},X)", instr.mnemonic, operand as u8), 2);
        }
        IndirectIndexed => {
            disasm = (format!("{0} (${1:X}),Y", instr.mnemonic, operand as u8), 2);
        }
    }

    disasm
}