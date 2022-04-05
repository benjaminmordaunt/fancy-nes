use std::{cell::RefCell, rc::Rc};

use crate::ppu::NESPPU;

use super::mapper::Mapper;

type IORegisters = [u8; 0x0018];
    /* SQ1_VOL */
    /* SQ1_SWEEP */
    /* SQ1_LO */
    /* SQ1_HI */
    /* SQ2_VOL */
    /* SQ2_SWEEP */
    /* SQ2_LO */
    /* SQ2_HI */
    /* TRI_LINEAR */
    /* (unused) */
    /* TRI_LO */
    /* TRI_HI */
    /* NOISE_VOL */
    /* (unused) */
    /* NOISE_LO */
    /* NOISE_HI */
    /* DMC_FREQ */
    /* DMC_RAW */
    /* DMC_START */
    /* DMC_LEN */
    /* OAM_DMA */
    /* SND_CHN */
    /* JOY1 */
    /* JOY2 */

pub struct CPUMemory {
    pub internal_ram: [u8; 0x0800],
    pub io_registers: IORegisters,
    pub cartridge_mapper: Box<dyn Mapper>,
    pub ppu_registers: Option<Rc<RefCell<NESPPU>>>,
}

impl CPUMemory {
    pub fn read(&mut self, addr: u16) -> u8 {
        /* Internal RAM */
        if (addr & 0xF000) < 0x2000 {
            return self.internal_ram[(addr & 0x07FF) as usize];
        }

        /* PPU control registers */
        if (addr & 0xF000) == 0x2000 || (addr & 0xF000) == 0x3000 {
            return self.ppu_registers.as_mut().unwrap().borrow_mut().ppu_register_read(addr);
        }

        /* APU and I/O */
        if (addr >= 0x4000) && (addr <= 0x4017) {
            return self.io_registers[(addr - 0x4000) as usize];
        }

        /* CPU test mode registers */
        if (addr >= 0x4018) && (addr <= 0x401F) {
            return 0;
        }

        /* Any address 0x4020 - 0xFFFF is handled by a mapper */
        if (addr >= 0x4020) && (addr <= 0xFFFF) {
            return self.cartridge_mapper.read(addr);
        }

        unimplemented!();
    }

    // Provide no side-effect read functions used by the debug string
    // generator module. i.e. accesses to memory-mapped registers are disallowed
    // (this will likely need to be constrained further once complex mappers are introduced)
    pub fn observe(&self, addr: u16) -> u8 {
        /* Internal RAM */
        if (addr & 0xF000) < 0x2000 {
            return self.internal_ram[(addr & 0x07FF) as usize];
        }

        /* PPU control registers */
        if (addr & 0xF000) == 0x2000 || (addr & 0xF000) == 0x3000 {
            panic!("Attempted read of address with side-affect from observer.")
        }

        /* APU and I/O */
        if (addr >= 0x4000) && (addr <= 0x4017) {
            return self.io_registers[(addr - 0x4000) as usize];
        }

        /* CPU test mode registers */
        if (addr >= 0x4018) && (addr <= 0x401F) {
            return 0;
        }

        /* Any address 0x4020 - 0xFFFF is handled by a mapper */
        if (addr >= 0x4020) && (addr <= 0xFFFF) {
            return self.cartridge_mapper.read(addr);
        }

        unimplemented!();
    }

    pub fn observe_16(&self, addr: u16) -> u16 {
        self.observe(addr) as u16
        | (self.observe(addr + 1) as u16) << 8
    }

    pub fn read_16(&mut self, addr: u16) -> u16 {
        self.read(addr) as u16
        | (self.read(addr + 1) as u16) << 8
    }

    pub fn write(&mut self, addr: u16, data: u8) {
        /* Internal RAM */
        if (addr & 0xF000) < 0x2000 {
            self.internal_ram[(addr & 0x07FF) as usize] = data;
        }

        /* PPU control registers */
        /* TODO - in reality these are PPU mapped and take effect */
        if (addr & 0xF000) == 0x2000 || (addr & 0xF000) == 0x3000 {
            self.ppu_registers.as_mut().unwrap().borrow_mut().ppu_register_write(addr, data);
        }

        /* APU and I/O */
        /* TODO - in reality these are PPU mapped and take effect */
        if (addr >= 0x4000) && (addr <= 0x4017) {
            self.io_registers[(addr - 0x4000) as usize] = data;
        }

        /* CPU test mode registers */
        if (addr >= 0x4018) && (addr <= 0x401F) {
            return;
        }

        /* Any address 0x4020 - 0xFFFF is handled by a mapper */
        if (addr >= 0x4020) && (addr <= 0xFFFF) {
            return self.cartridge_mapper.write(addr, data);
        }
    }
}