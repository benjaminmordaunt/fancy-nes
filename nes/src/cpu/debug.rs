/// Provide the facilities necessary for the nes-platform
/// crate to generate a disasm view of the current NES PRG.

use super::{decode::Instruction, AddressingMode};

pub fn disasm_6502(instr: &Instruction, operand: u16) -> String {
    use AddressingMode::*;
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