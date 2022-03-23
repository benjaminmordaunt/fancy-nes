use bitflags::bitflags;

use self::decode::LUT_6502;

mod decode;

bitflags! {
    struct StatusRegister: u32 {
        const CARRY             = 0b00000001;
        const ZERO              = 0b00000010;
        const INTERRUPT_DISABLE = 0b00000100;
        const DECIMAL_MODE      = 0b00001000;
        const BREAK_COMMAND     = 0b00010000;
        const OVERFLOW          = 0b00100000;
        const NEGATIVE          = 0b01000000;
    }
}

enum AddressingMode {
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

struct Cpu {
    status: StatusRegister,
    PC: u16,    /* program counter */
    SP: u8,     /* stack pointer */
    A: u8,      /* accumulator */
    X: u8,      /* index register X */
    Y: u8,      /* index register Y */

    /* instructions */
    target_address: u16,  /* "address" dumped straight from operand */
    wait_cycles: u8,      /* pending wait cycles */
}

impl Cpu {
    fn do_op(&mut self) {
        /* Fetch stage */
        let op = self.memory.read(self.PC);
        let instr = LUT_6502[op];

        /* Execute stage */
        match instr.mnemonic {
            "ADC" => self.A = self.op_arithmetic(&instr.mode, true),
            "AND" => self.A = self.op_bitwise(&instr.mode, |x, y| { x & y }),
            "LDA" => self.A = self.op_load(&instr.mode),
            "LDX" => self.X = self.op_load(&instr.mode),
            "LDY" => self.Y = self.op_load(&instr.mode),
            "SBC" => self.A = self.op_arithmetic(&instr.mode, false),
        }

        /* Wait a specific # of cycles */
        while instr.cycles > 0 {
            self.spin();
            instr.cycles -= 1;
        }
    
        /* Wait more cycles depending on instruction execution */
        while self.wait_cycles > 0 {
            self.spin();
            instr.cycles -= 1;
        }
    }

    /* resolve the address presented in the operand in
       accorance with addressing mode rules */
    fn resolve_address(&self, mode: &AddressingMode) -> (u16, bool) {
        match mode {
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
                return (self.memory.read(zp_addr), false);
            }
            AddressingMode::IndirectIndexed => {
                return (self.memory.read(self.target_address) + self.Y as u16, false);
            }
            _ => unimplemented!()
        }
    }

    /* arithmetic operations - ADC, SBC */
    fn op_arithmetic(&mut self, mode: &AddressingMode, add: bool) -> u8 {
        let (addr, page_cross) = self.resolve_address(mode);
        let data = self.memory.read(addr);

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
        self.memory.write(data);
    }

    /* jump operations - JMP, JSR, RTI, RTS */
    fn op_jump(&mut self, mode: &AddressingMode) {
        let (addr, _) = self.resolve_address(mode);

        self.PC = addr;
    }

    /* conditional branch operations - BMI, BEQ, BNE, BPL, BVC, BVS */
    fn op_branch(&mut self, reg: StatusRegister, set: bool, mode: &AddressingMode) {
        if self.status.contains(reg) == set {
            let (addr, page_cross) = self.resolve_address(mode);
            if page_cross {
                self.wait_cycles += 1;
            }
            self.PC = addr;
        }
    }

    /* Bitwise operators - AND, EOR, ORA */
    fn op_bitwise(&mut self, mode: &AddressingMode, func: impl Fn(u8, u8) -> u8) -> u8 {
        let (addr, page_cross) = self.resolve_address(mode);
        let data = self.memory.read(addr);

        let result = func(self.A, data);
        self.status.set(StatusRegister::ZERO, result == 0);
        self.status.set(StatusRegister::NEGATIVE, result & 0x80 > 0);
        result
    }

    /* Register transfers - TAX, TXA, TAY, TYA, TSX, TXS */
    fn op_transfer_a(&mut self, from: u8, txs: bool) -> u8 {
        if !txs {
            self.status.set(StatusRegister::ZERO, self.A == 0);
            self.status.set(StatusRegister::NEGATIVE, self.A & 0b10000000 > 0);
        }
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

    /* Set/clear interrupt disable flag - SEI, CLI */
    fn op_interrupt_disable(&mut self, set: bool) {
        self.status.set(StatusRegister::INTERRUPT_DISABLE, set);
    }

    /* Set/clear decimal mode flag - SED, CLD */
    fn op_decimal_mode(&mut self, set: bool) {
        /* Some (Famicom) games require decimal mode: Duck Maze (original Bit Corp. ed.)
           and Othello. These are very naughty boys and decimal mode is not supported. */
        unimplemented!()
    }

    /* Clear overflow flag - CLV */
    fn op_overflow(&mut self) {
        self.status.set(StatusRegister::OVERFLOW, false);
    }

}
