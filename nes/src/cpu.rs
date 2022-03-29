use std::cell::RefCell;

use bitflags::bitflags;

use self::decode::{LUT_6502, Instruction};
use self::mapper000::Mapper000;
use self::mem::*;

pub mod decode;
pub mod debug;
mod mem;

// Mappers
mod mapper;
mod mapper000;

/* The BREAK flag(s) is only applicable when the
   status register is pushed to the stack. 
   Programs can query BREAK_LOW to determine whether
   they are in a soft (BRK,"PHP") or hard (IRQ,NMI)
   interrupt. Though, it's not particularly useful
   because an NMI can fire during BRK vector routine
   (not emulated). BREAK_HIGH is always 1. */
bitflags! {
    struct StatusRegister: u8 {
        const CARRY             = 0b00000001;
        const ZERO              = 0b00000010;
        const INTERRUPT_DISABLE = 0b00000100;
        const DECIMAL_MODE      = 0b00001000;
        const BREAK_LOW         = 0b00010000; /* see above */
        const BREAK_HIGH        = 0b00100000; /* see above */
        const OVERFLOW          = 0b01000000;
        const NEGATIVE          = 0b10000000;
    }
}

enum InterruptType {
    SUBROUTINE, /* not an interrupt at all */
    BRK,        /* software interrupt */
    IRQ,        /* hard interrupt request */
    NMI,        /* non-maskable interrupt (from PPU) */
}

pub enum AddressingMode {
    Implied,
    Accumulator,
    Immediate,
    ZeroPage,
    ZeroPageX,
    ZeroPageY,
    Relative,
    Absolute,
    AbsoluteX,
    AbsoluteY,
    Indirect,
    IndexedIndirect,
    IndirectIndexed,
}

pub struct NESCpu {
    status: StatusRegister,
    pub PC: u16,    /* program counter */
    SP: u8,     /* stack pointer */
    A: u8,      /* accumulator */
    X: u8,      /* index register X */
    Y: u8,      /* index register Y */

    /* instructions */
    target_address: u16,  /* "address" dumped straight from operand */
    wait_cycles: u8,      /* pending wait cycles */
    pc_skip: u16,           /* how many bytes to advance the PC by for a given instr. */

    pub memory: CPUMemory,
}

impl NESCpu {
    pub fn new(mapper_id: usize) -> Self {
        Self {
            status: StatusRegister::empty(),
            PC: 0, /* given a correct value from the reset method  */
            SP: 0, /* given a correct value by the ROM's init code */
            A: 0,
            X: 0,
            Y: 0,
            target_address: 0,
            wait_cycles: 0,
            pc_skip: 0,
            memory: CPUMemory {
                internal_ram: [0; 2048],
                ppu_registers: [0; 8],
                io_registers: [0; 24],
                cartridge_mapper: Box::new(
                    match mapper_id {
                        0 => {
                            Mapper000::new()
                        }
                        _ => unimplemented!()
                    }
                )
            }
        }
    }

    pub fn do_op(&mut self) {
        /* Fetch stage */
        let op = self.memory.read(self.PC);
        let instr_opt = LUT_6502.get(&op);
        let instr: &Instruction;

        if instr_opt.is_none() {
            println!("No instruction found for opcode ${:X}", op);
            return;
        }

        instr = instr_opt.unwrap();

        /* Execute stage */
        match instr.mnemonic {
            "ADC" => self.A = self.op_arithmetic(&instr.mode, true),
            "AND" => self.A = self.op_bitwise(&instr.mode, |x, y| { x & y }),
            "ASL" => self.op_rotate(&instr.mode, true, true),
            "BCC" => self.op_branch(StatusRegister::CARRY, false, &instr.mode),
            "BCS" => self.op_branch(StatusRegister::CARRY, true, &instr.mode),
            "BEQ" => self.op_branch(StatusRegister::ZERO, true, &instr.mode),
            "BIT" => self.op_bit(&instr.mode),
            "BMI" => self.op_branch(StatusRegister::NEGATIVE, true, &instr.mode),
            "BNE" => self.op_branch(StatusRegister::ZERO, false, &instr.mode),
            "BPL" => self.op_branch(StatusRegister::NEGATIVE, false, &instr.mode),
            "BRK" => self.enter_subroutine(&InterruptType::BRK),
            "BVC" => self.op_branch(StatusRegister::OVERFLOW, false, &instr.mode),
            "BVS" => self.op_branch(StatusRegister::OVERFLOW, true, &instr.mode),
            "CLC" => self.status.set(StatusRegister::CARRY, false),
            "CLD" => self.status.set(StatusRegister::DECIMAL_MODE, false),
            "CLI" => self.status.set(StatusRegister::INTERRUPT_DISABLE, false),
            "CLV" => self.status.set(StatusRegister::OVERFLOW, false),
            "CMP" => self.op_compare(self.A, &instr.mode),
            "CPX" => self.op_compare(self.X, &instr.mode),
            "CPY" => self.op_compare(self.Y, &instr.mode),
            "DEC" => self.A = self.op_incdec(self.A, false),
            "DEX" => self.X = self.op_incdec(self.X, false),
            "DEY" => self.Y = self.op_incdec(self.Y, false),
            "EOR" => self.A = self.op_bitwise(&instr.mode, |x, y| { x ^ y }),
            "INC" => self.A = self.op_incdec(self.A, true),
            "INX" => self.X = self.op_incdec(self.X, true),
            "INY" => self.Y = self.op_incdec(self.Y, true),
            "JMP" => self.op_jump(&instr.mode),
            "JSR" => self.enter_subroutine(&InterruptType::SUBROUTINE),
            "LDA" => self.A = self.op_load(&instr.mode),
            "LDX" => self.X = self.op_load(&instr.mode),
            "LDY" => self.Y = self.op_load(&instr.mode),
            "LSR" => self.op_rotate(&instr.mode, false, true),
            "NOP" => {},
            "ORA" => self.A = self.op_bitwise(&instr.mode, |x, y| { x | y }),
            "PHA" => self.op_stack_push(false),
            "PHP" => self.op_stack_push(true),
            "PLA" => self.A = self.op_stack_pull(false),
            "PLP" => self.status = StatusRegister::from_bits_truncate(self.op_stack_pull(true)),
            "ROL" => self.op_rotate(&instr.mode, true, false),
            "ROR" => self.op_rotate(&instr.mode, false, false),
            "RTI" => self.leave_subroutine(&InterruptType::IRQ),
            "RTS" => self.leave_subroutine(&InterruptType::SUBROUTINE),
            "SBC" => self.A = self.op_arithmetic(&instr.mode, false),
            "SEC" => { self.status.set(StatusRegister::CARRY, true); self.pc_skip = 1; },
            "SED" => { self.status.set(StatusRegister::DECIMAL_MODE, true); self.pc_skip = 1; },
            "SEI" => { self.status.set(StatusRegister::INTERRUPT_DISABLE, true); self.pc_skip = 1; },
            "STA" => self.op_store(self.A, &instr.mode),
            "STX" => self.op_store(self.X, &instr.mode),
            "STY" => self.op_store(self.Y, &instr.mode),
            "TAX" => self.X = self.op_transfer_a(self.A, false),
            "TAY" => self.Y = self.op_transfer_a(self.A, false),
            "TSX" => self.X = self.op_transfer_a(self.SP, false),
            "TXS" => self.SP = self.op_transfer_a(self.X, true),
            "TYA" => self.A = self.op_transfer_a(self.Y, false),
            _     => unimplemented!()
        }
        
        self.PC += self.pc_skip;
    }

    /* resolve the address presented in the operand in
       accorance with addressing mode rules */
    fn resolve_address(&mut self, mode: &AddressingMode) -> (u16, bool) {

        /* Set the target_address based on the command length */
        match mode {
            AddressingMode::ZeroPage |
            AddressingMode::ZeroPageX |
            AddressingMode::ZeroPageY |
            AddressingMode::IndirectIndexed |
            AddressingMode::IndexedIndirect => {
                self.target_address = self.memory.read(self.PC + 1) as u16;
                self.pc_skip = 2;
            },
            AddressingMode::Absolute |
            AddressingMode::AbsoluteX |
            AddressingMode::AbsoluteY |
            AddressingMode::Indirect => {
                self.target_address = self.memory.read_16(self.PC + 1);
                self.pc_skip = 3;
            },
            AddressingMode::Immediate => {
                self.pc_skip = 2;
            }
            _ => { self.pc_skip = 1; }
        }

        match mode {
            AddressingMode::Immediate => {
                return (self.PC + 1, false);
            },
            AddressingMode::ZeroPage => {
                return (self.target_address & 0xFF, false);
            },
            AddressingMode::ZeroPageX => {
                return (((self.target_address & 0xFF) + self.X as u16) & 0xFF, false); 
            }
            AddressingMode::ZeroPageY => {
                return (((self.target_address & 0xFF) + self.Y as u16) & 0xFF, false);
            }
            AddressingMode::Relative => {
                return (self.PC.wrapping_add((self.target_address as i8) as u16), false);
            }
            AddressingMode::Absolute => {
                return (self.target_address, false);
            }
            AddressingMode::AbsoluteX => {
                return (self.target_address + self.X as u16, false);
            }
            AddressingMode::AbsoluteY => {
                return (self.target_address + self.Y as u16, false);
            }
            AddressingMode::Indirect => {
                let addr_lsb: u8 = self.memory.read(self.target_address);
                let addr_msb: u8 = self.memory.read((self.target_address + 1) & 0xFF00);

                 /* An original 6502 has does not correctly fetch 
                    the target address if the indirect vector falls on a page boundary (e.g. $xxFF where 
                    xx is any value from $00 to $FF). In this case fetches the LSB from $xxFF as expected 
                    but takes the MSB from $xx00. This is fixed in some later chips like the 65SC02 so 
                    for compatibility always ensure the indirect vector is not at the end of the page.
                 */
                #[cfg(debug_assertions)]
                if addr_lsb == 0xFF {
                    println!("Indirect JMP at ${:X} falls at end of page. Using \"broken\" behaviour.", &self.PC);
                }
                return (addr_lsb as u16 + (addr_msb as u16) << 4, false);
            }
            AddressingMode::IndexedIndirect => {
                let zp_addr: u16 = (self.target_address as u8 + self.X) as u16;
                return (self.memory.read_16(zp_addr), false);
            }
            AddressingMode::IndirectIndexed => {
                return (self.memory.read_16(self.target_address) + self.Y as u16, false);
            }
            _ => unimplemented!()
        }
    }

    /* arithmetic operations - ADC, SBC */
    fn op_arithmetic(&mut self, mode: &AddressingMode, add: bool) -> u8 {
        let (addr, page_cross) = self.resolve_address(mode);
        let mut data = self.memory.read(addr);

        /* Interestingly, a simple one's complement works here, including all flags
           (exercise for the reader :-) ) */
        if !add {
            data = !data;
        }

        let (result, carry_data) = self.A.overflowing_add(data);
        let (result, carry_cin) = result.overflowing_add(self.status.contains(StatusRegister::CARRY) as u8);

        self.status.set(StatusRegister::CARRY, carry_data || carry_cin);
        self.status.set(StatusRegister::ZERO, result == 0);
        self.status.set(StatusRegister::OVERFLOW, (self.A ^ result) & (data ^ result) & 0x80 != 0);
        self.status.set(StatusRegister::NEGATIVE, result & 0x80 > 0);

        if page_cross {
            self.wait_cycles += 1;
        }
        result
    }

    /* load operations - LDA, LDX, LDY */
    fn op_load(&mut self, mode: &AddressingMode) -> u8 {
        let (addr, page_cross) = self.resolve_address(mode);
        let data = self.memory.read(addr);
        self.status.set(StatusRegister::ZERO, data == 0);
        self.status.set(StatusRegister::NEGATIVE, data & 0b10000000 > 0);
        if page_cross {
            self.wait_cycles += 1;
        }
        data
    }

    /* store operations - STA, STX, STY */
    fn op_store(&mut self, data: u8, mode: &AddressingMode) {
        let (addr, _) = self.resolve_address(mode);
        self.memory.write(addr, data);
    }

    /* jump operations - JMP, JSR, RTI, RTS */
    fn op_jump(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.resolve_address(mode);

        self.PC = addr;
    }

    /* bit test */
    fn op_bit(&mut self, mode: &AddressingMode) {
        let addr = self.resolve_address(mode).0;
        let data = self.memory.read(addr);

        self.status.set(StatusRegister::ZERO, self.A & data == 0);
        self.status.set(StatusRegister::OVERFLOW, data & 0x40 > 0);
        self.status.set(StatusRegister::NEGATIVE, data & 0x80 > 0);
    }

    /* conditional branch operations - BMI, BEQ, BNE, BPL, BVC, BVS */
    fn op_branch(&mut self, reg: StatusRegister, set: bool, mode: &AddressingMode) {
        let (addr, page_cross) = self.resolve_address(mode);
        if self.status.contains(reg) == set {
            if page_cross {
                self.wait_cycles += 1;
            }
            self.PC = addr;
        }
        self.pc_skip = 0;
    }

    /* Bitwise operators - AND, EOR, ORA */
    fn op_bitwise(&mut self, mode: &AddressingMode, func: impl Fn(u8, u8) -> u8) -> u8 {
        let (addr, _) = self.resolve_address(mode);
        let data = self.memory.read(addr);

        let result = func(self.A, data);
        self.status.set(StatusRegister::ZERO, result == 0);
        self.status.set(StatusRegister::NEGATIVE, result & 0x80 > 0);
        result
    }

    /* Increment/decrement operators - INC, INX, INY, DEC, DEX, DEY */
    fn op_incdec(&mut self, data: u8, inc: bool) -> u8 {
        let result = if inc { data + 1 } else { data - 1 };
        self.status.set(StatusRegister::ZERO, result == 0);
        self.status.set(StatusRegister::NEGATIVE, result & 0x80 > 0);

        self.pc_skip = 1;
        result
    }

    /* Rotate operators - ROL, ROR */
    fn op_rotate(&mut self, mode: &AddressingMode, left: bool, arith: bool) {
        let addr = self.resolve_address(mode).0;

        let mut data = if matches!(mode, AddressingMode::Accumulator) {
            self.A
        } else {
            self.memory.read(addr)
        };

        let old_carry = self.status.contains(StatusRegister::CARRY) as u8;
        if left {
            self.status.set(StatusRegister::CARRY, data & 0x80 > 0);
            data = data.rotate_left(1);
            data = (data & 0b11111110) | (if arith { 0 } else { old_carry }); 
            self.status.set(StatusRegister::NEGATIVE, data & 0x80 > 0);
        } else {
            self.status.set(StatusRegister::CARRY, data & 0x1 > 0);
            data = data.rotate_right(1);
            data = (data & 0b01111111) | (if arith { 0 } else {old_carry << 7});
            self.status.set(StatusRegister::NEGATIVE, data * 0x80 > 0);
        }

        if matches!(mode, AddressingMode::Accumulator) {
            self.A = data;
        } else {
            self.memory.write(addr, data);
        }
    }

    /* Register transfers - TAX, TXA, TAY, TYA, TSX, TXS */
    fn op_transfer_a(&mut self, from: u8, txs: bool) -> u8 {
        if !txs {
            self.status.set(StatusRegister::ZERO, self.A == 0);
            self.status.set(StatusRegister::NEGATIVE, self.A & 0b10000000 > 0);
        }

        self.pc_skip = 1;
        from
    }

    /* Comparison instructions - CMP, CPX, CPY */
    fn op_compare(&mut self, lhs: u8, mode: &AddressingMode) {
        let (addr, page_cross) = self.resolve_address(mode);
        let rhs = self.memory.read(addr);

        self.status.set(StatusRegister::CARRY, lhs >= rhs);
        self.status.set(StatusRegister::ZERO, lhs == rhs);
        self.status.set(StatusRegister::NEGATIVE, lhs.wrapping_sub(rhs) & 0x80 > 0);
        if page_cross {
            self.wait_cycles += 1;
        }
    }

    /* Stack operations - PHA, PHP, PLA, PLP */
    fn op_stack_push(&mut self, status: bool) {
        if status {
            self.memory.write(self.SP as u16 + 0x0100, self.status.bits());
        } else {
            self.memory.write(self.SP as u16 + 0x0100, self.A);
        }
        self.SP -= 1;
        self.pc_skip = 1;
    }

    fn op_stack_pull(&mut self, status: bool) -> u8 {
        self.SP -= 1;
        self.pc_skip = 1;
        if status {
            return self.memory.read(self.SP as u16 + 0x0100);
        } else {
            let result = self.memory.read(self.SP as u16 + 0x0100);
            self.status.set(StatusRegister::ZERO, result == 0);
            self.status.set(StatusRegister::NEGATIVE, result & 0x80 > 0);
            return result;
        }
    }

    /* branch to interrupt or subroutine */
    fn enter_subroutine(&mut self, inttype: &InterruptType) {
        
        /* if we've ended up here to do an IRQ service when
           interrupt disable is set, do nothing */
        if matches!(inttype, InterruptType::IRQ) && self.status.contains(StatusRegister::INTERRUPT_DISABLE) {
            return;
        }

        /* if a software BRK has been called, allow for single
           byte patching by setting the stacked address to the one subsequent */
        if matches!(inttype, InterruptType::BRK) {
            self.PC += 1;
        }

        self.memory.write(self.SP as u16 + 0x0100, (self.PC >> 8) as u8); /* PC, MSB */
        self.SP -= 1;
        self.memory.write(self.SP as u16 + 0x0100, self.PC as u8); /* PC, LSB */
        self.SP -= 1;
        match inttype {
            InterruptType::SUBROUTINE => {
                self.PC = self.memory.read_16(self.PC + 1);
            },
            InterruptType::BRK => {
                self.status.insert(StatusRegister::BREAK_LOW);
                self.memory.write(self.SP as u16 + 0x0100, self.status.bits());
                self.status.insert(StatusRegister::INTERRUPT_DISABLE);
                self.SP -= 1;
                self.PC = self.memory.read_16(0xFFFA);
            },
            InterruptType::IRQ => {
                self.status.remove(StatusRegister::BREAK_LOW);
                self.memory.write(self.SP as u16 + 0x0100, self.status.bits());
                self.status.insert(StatusRegister::INTERRUPT_DISABLE);
                self.SP -= 1;
                self.PC = self.memory.read_16(0xFFFE);
            },
            InterruptType::NMI => {
                self.status.remove(StatusRegister::BREAK_LOW);
                self.memory.write(self.SP as u16 + 0x0100, self.status.bits());
                self.status.insert(StatusRegister::INTERRUPT_DISABLE);
                self.SP -= 1;
                self.PC = self.memory.read_16(0xFFFA);
            }
        }

        self.pc_skip = 0;
    }

    /* return from a subroutine or interrupt */
    fn leave_subroutine(&mut self, inttype: &InterruptType) {
        let mut pc: u16 = 0;

        match inttype {
            InterruptType::IRQ
            | InterruptType::BRK
            | InterruptType::NMI => {
                self.SP += 1;
                self.status = StatusRegister::from_bits_truncate(self.memory.read(self.SP as u16 + 0x0100));
                self.status.remove(StatusRegister::INTERRUPT_DISABLE);
            }
            _ => {}
        }

        self.SP += 1;
        pc |= self.memory.read(self.SP as u16 + 0x0100) as u16;
        self.SP += 1;
        pc |= (self.memory.read(self.SP as u16 + 0x0100) as u16) << 8;

        /* Actually start at the next instruction */
        pc += 1;
        self.PC = pc;

        self.pc_skip = 0;
    }

    /* The NES's reset signal handling */
    pub fn reset(&mut self) {
        self.status.insert(StatusRegister::INTERRUPT_DISABLE);
        self.status.insert(StatusRegister::BREAK_HIGH); /* always 1 */
        self.PC = self.memory.read_16(0xFFFC);
    }

    /* Handle the NMI (non-maskable interrupt) - called primarily by the PPU */
    fn nmi(&mut self) {
        self.memory.write(self.SP as u16 + 0x0100, (self.PC >> 8) as u8); /* PC, MSB */
        self.SP -= 1;
        self.memory.write(self.SP as u16 + 0x0100, self.PC as u8); /* PC, LSB */
        self.SP -= 1;
        self.status.remove(StatusRegister::BREAK_LOW); /* NMI is a hard interrupt */
        self.memory.write(self.SP as u16 + 0x0100, self.status.bits()); /* Status */
        self.SP -= 1;
        self.status.set(StatusRegister::INTERRUPT_DISABLE, true);
        self.PC = self.memory.read(0xFFEA) as u16 +
                (self.memory.read(0xFFEB) as u16) << 8;
    }
}
