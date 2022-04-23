extern crate lazy_static;

use std::{collections::HashMap, vec};
use lazy_static::lazy_static;
use super::AddressingMode;

#[derive(Clone, Copy)]
pub struct Instruction {
    pub mnemonic: &'static str,
    pub mode: AddressingMode,
    pub cycles: u16,
}

lazy_static! {
    pub static ref LUT_6502: HashMap<u8, Instruction> = {
        let mut lut = HashMap::new();
        let mut add = |ops_str, ops: Vec<(u8, AddressingMode, u16)>| {
            for op in ops {
                lut.insert(op.0, Instruction { mnemonic: ops_str, mode: op.1, cycles: op.2 });
            }
        };

        /* We're going to need shorter aliases for that large LUT! */
        use AddressingMode::Immediate as IMM;
        use AddressingMode::Implied as IMP;
        use AddressingMode::Accumulator as ACC;
        use AddressingMode::ZeroPage as ZP;
        use AddressingMode::ZeroPageX as ZPX;
        use AddressingMode::ZeroPageY as ZPY;
        use AddressingMode::Relative as REL;
        use AddressingMode::Absolute as ABS;
        use AddressingMode::AbsoluteX as ABX;
        use AddressingMode::AbsoluteY as ABY;
        use AddressingMode::Indirect as IND;
        use AddressingMode::IndexedIndirect as IDI;
        use AddressingMode::IndirectIndexed as IID;

        add("ADC", vec![(0x69, IMM, 2), (0x65, ZP, 3), (0x75, ZPX, 4), (0x6D, ABS, 4),
                (0x7D, ABX, 4), (0x79, ABY, 4), (0x61, IDI, 6), (0x71, IID, 5)]);
        add("AND", vec![(0x29, IMM, 2), (0x25, ZP, 3), (0x35, ZPX, 4), (0x2D, ABS, 4),
                (0x3D, ABX, 4), (0x39, ABY, 4), (0x21, IDI, 6), (0x31, IID, 5)]);
        add("ASL", vec![(0x0A, ACC, 2), (0x06, ZP, 5), (0x16, ZPX, 6), (0x0E, ABS, 6), 
                (0x1E, ABX, 7)]);
        add("BCC", vec![(0x90, REL, 2)]);
        add("BCS", vec![(0xB0, REL, 2)]);
        add("BEQ", vec![(0xF0, REL, 2)]);
        add("BIT", vec![(0x24, ZP, 3), (0x2C, ABS, 4)]);
        add("BMI", vec![(0x30, REL, 2)]);
        add("BNE", vec![(0xD0, REL, 2)]);
        add("BPL", vec![(0x10, REL, 2)]);
        add("BRK", vec![(0x00, IMP, 7)]);
        add("BVC", vec![(0x50, REL, 2)]);
        add("BVS", vec![(0x70, REL, 2)]);
        add("CLC", vec![(0x18, IMP, 2)]);
        add("CLD", vec![(0xD8, IMP, 2)]);
        add("CLI", vec![(0x58, IMP, 2)]);
        add("CLV", vec![(0xB8, IMP, 2)]);
        add("CMP", vec![(0xC9, IMM, 2), (0xC5, ZP, 3), (0xD5, ZPX, 4), (0xCD, ABS, 4),
                (0xDD, ABX, 4), (0xD9, ABY, 4), (0xC1, IDI, 6), (0xD1, IID, 5)]);
        add("CPX", vec![(0xE0, IMM, 2), (0xE4, ZP, 3), (0xEC, ABS, 4)]);
        add("CPY", vec![(0xC0, IMM, 2), (0xC4, ZP, 3), (0xCC, ABS, 4)]);
        add("DEC", vec![(0xC6, ZP, 5), (0xD6, ZPX, 6), (0xCE, ABS, 6), (0xDE, ABX, 7)]);
        add("DEX", vec![(0xCA, IMP, 2)]);
        add("DEY", vec![(0x88, IMP, 2)]);
        add("EOR", vec![(0x49, IMM, 2), (0x45, ZP, 3), (0x55, ZPX, 4), (0x4D, ABS, 4),
                (0x5D, ABX, 4), (0x59, ABY, 3), (0x41, IDI, 6), (0x51, IID, 5)]);
        add("INC", vec![(0xE6, ZP, 5), (0xF6, ZPX, 6), (0xEE, ABS, 6), (0xFE, ABX, 7)]);
        add("INX", vec![(0xE8, IMP, 2)]);
        add("INY", vec![(0xC8, IMP, 2)]);
        add("JMP", vec![(0x4C, ABS, 3), (0x6C, IND, 5)]);
        add("JSR", vec![(0x20, ABS, 6)]);
        add("LDA", vec![(0xA9, IMM, 2), (0xA5, ZP, 3), (0xB5, ZPX, 4), (0xAD, ABS, 4),
                (0xBD, ABX, 4), (0xB9, ABY, 4), (0xA1, IDI, 6), (0xB1, IID, 5)]);
        add("LDX", vec![(0xA2, IMM, 2), (0xA6, ZP, 3), (0xB6, ZPY, 4), (0xAE, ABS, 4),
                (0xBE, ABY, 4)]);
        add("LDY", vec![(0xA0, IMM, 2), (0xA4, ZP, 3), (0xB4, ZPX, 4), (0xAC, ABS, 4),
                (0xBC, ABX, 4)]);
        add("LSR", vec![(0x4A, ACC, 2), (0x46, ZP, 5), (0x56, ZPX, 6), (0x4E, ABS, 6),
                (0x5E, ABX, 7)]);
        add("NOP", vec![(0xEA, IMP, 2)]);
        add("ORA", vec![(0x09, IMM, 2), (0x05, ZP, 3), (0x15, ZPX, 4), (0x0D, ABS, 4),
                (0x1D, ABX, 4), (0x19, ABY, 4), (0x01, IDI, 6), (0x11, IID, 5)]);
        add("PHA", vec![(0x48, IMP, 3)]);
        add("PHP", vec![(0x08, IMP, 3)]);
        add("PLA", vec![(0x68, IMP, 4)]);
        add("PLP", vec![(0x28, IMP, 4)]);
        add("ROL", vec![(0x2A, ACC, 2), (0x26, ZP, 5), (0x36, ZPX, 6), (0x2E, ABS, 6),
                (0x3E, ABX, 7)]);
        add("ROR", vec![(0x6A, ACC, 2), (0x66, ZP, 5), (0x76, ZPX, 6), (0x6E, ABS, 6),
                (0x7E, ABX, 7)]);
        add("RTI", vec![(0x40, IMP, 6)]);
        add("RTS", vec![(0x60, IMP, 6)]);
        add("SBC", vec![(0xE9, IMM, 2), (0xE5, ZP, 3), (0xF5, ZPX, 4), (0xED, ABS, 4),
                (0xFD, ABX, 4), (0xF9, ABY, 4), (0xE1, IDI, 6), (0xF1, IID, 5)]);
        add("SEC", vec![(0x38, IMP, 2)]);
        add("SED", vec![(0xF8, IMP, 2)]);
        add("SEI", vec![(0x78, IMP, 2)]);
        add("STA", vec![(0x85, ZP, 3), (0x95, ZPX, 4), (0x8D, ABS, 4), (0x9D, ABX, 5),
                (0x99, ABY, 5), (0x81, IDI, 6), (0x91, IID, 6)]);
        add("STX", vec![(0x86, ZP, 3), (0x96, ZPY, 4), (0x8E, ABS, 4)]);
        add("STY", vec![(0x84, ZP, 3), (0x94, ZPX, 4), (0x8C, ABS, 4)]);
        add("TAX", vec![(0xAA, IMP, 2)]);
        add("TAY", vec![(0xA8, IMP, 2)]);
        add("TSX", vec![(0xBA, IMP, 2)]);
        add("TXA", vec![(0x8A, IMP, 2)]);
        add("TXS", vec![(0x9A, IMP, 2)]);
        add("TYA", vec![(0x98, IMP, 2)]);

        /* TODO: Illegal opcodes */
        lut
    };
}