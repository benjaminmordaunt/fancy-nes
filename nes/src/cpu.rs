struct cpu {
    status: u8, /* status register */
    PC: u16,    /* program counter */
    SP: u8,     /* stack pointer */
    A: u8,      /* accumulator */
    X: u8,      /* index register X */
    Y: u8       /* index register Y */
}

impl cpu {
    fn carry(&mut self, c: bool) {
        self.status = (self.status & 0b10111111) | ((c as u8) << 6);
    }
    fn zero(&mut self, z: bool) {
        self.status = (self.status & 0b11011111) | ((z as u8) << 5);
    }
    fn interrupt_disable(&mut self, i: bool) {
        self.status = (self.status & 0b11101111) | ((i as u8) << 4);
    }
    fn decimal_mode(&mut self, d: bool) {
        self.status = (self.status & 0b11110111) | ((d as u8) << 3);
    }
    fn break_command(&mut self, b: bool) {
        self.status = (self.status & 0b11111011) | ((b as u8) << 2);
    }
    fn overflow(&mut self, o: bool) {
        self.status = (self.status & 0b11111101) | ((o as u8) << 1);
    }
    fn negative(&mut self, n: bool) {
        self.status = (self.status & 0b11111110) | ((n as u8) << 0);
    }
}
