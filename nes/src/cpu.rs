struct cpu_status {
    reg: u8,
    PC: u16, /* program counter */
    SP: u8,  /* stack pointer */
    A: u8,   /* accumulator */
    X: u8,   /* index register X */
    Y: u8    /* index register Y */
}

impl cpu_status {
    fn carry(&self, c: bool) {
        self.reg = (self.reg & 0b10111111) | ((c as u8) << 6);
    }
    fn zero(&self, z: bool) {
        self.reg = (self.reg & 0b11011111) | ((z as u8) << 5);
    }
    fn interrupt_disable(&self, i: bool) {
        self.reg = (self.reg & 0b11101111) | ((i as u8) << 4);
    }
    fn decimal_mode(&self, d: bool) {
        self.reg = (self.reg & 0b11110111) | ((d as u8) << 3);
    }
    fn break_command(&self, b: bool) {
        self.reg = (self.reg & 0b11111011) | ((b as u8) << 2);
    }
    fn overflow(&self, o: bool) {
        self.reg = (self.reg & 0b11111101) | ((o as u8) << 1);
    }
    fn negative(&self, n: bool) {
        self.reg = (self.reg & 0b11101111) | ((n as u8) << 0);
    }
}
