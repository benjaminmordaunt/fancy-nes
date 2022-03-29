use crate::cpu::decode::{LUT_6502, Instruction};

/// Provide the facilities necessary for the nes-platform
/// crate to generate a disasm view of the current NES PRG.

use super::{AddressingMode, mem::CPUMemory};

pub fn disasm_6502(instruction_addr: u16, mem: &mut CPUMemory ) -> String {
    use AddressingMode::*;

    let opcode = &mem.read(instruction_addr);
    let instr_opt = LUT_6502.get(opcode);
    let instr: &Instruction;
    let operand: u16;

    if instr_opt.is_none() {
        return format!("Unknown disassembly for opcode {:X}", opcode);
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
            operand = mem.read(instruction_addr + 1) as u16;
        },
        AddressingMode::Absolute |
        AddressingMode::AbsoluteX |
        AddressingMode::AbsoluteY |
        AddressingMode::Indirect => {
            operand = mem.read_16(instruction_addr + 1);
        },
        _ => { operand = 0xDEAD; }
    }
    let disasm: String;

    match instr.mode {
        Accumulator => {
            disasm = format!("{0}", instr.mnemonic);
        }
        Implied => {
            disasm = format!("{0}", instr.mnemonic);
        }
        Immediate => {
            disasm = format!("{0} #${1:X}", instr.mnemonic, operand as u8);
        }
        Absolute => {
            disasm = format!("{0} ${1:X}", instr.mnemonic, operand);
        }
        ZeroPage | Relative => {
            disasm = format!("{0} ${1:X}", instr.mnemonic, operand as u8);
        }
        ZeroPageX => {
            disasm = format!("{0} ${1:X},X", instr.mnemonic, operand as u8);
        }
        ZeroPageY => {
            disasm = format!("{0} ${1:X},Y", instr.mnemonic, operand as u8);
        }
        Indirect => {
            disasm = format!("{0} (${1:X})", instr.mnemonic, operand);
        }
        AbsoluteX => {
            disasm = format!("{0} ${1:X},X", instr.mnemonic, operand);
        }
        AbsoluteY => {
            disasm = format!("{0} ${1:X},Y", instr.mnemonic, operand);
        }
        IndexedIndirect => {
            disasm = format!("{0} (${1:X},X)", instr.mnemonic, operand as u8);
        }
        IndirectIndexed => {
            disasm = format!("{0} (${1:X}),Y", instr.mnemonic, operand as u8);
        }
    }

    disasm
}