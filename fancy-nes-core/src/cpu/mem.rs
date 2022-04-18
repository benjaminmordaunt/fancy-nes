use std::{cell::RefCell, rc::Rc};
use std::ops::Deref;

use crate::ppu::NESPpu;

use super::mapper::Mapper;


pub trait MemoryRead {
    fn read(&self, addr: u16) -> u8;           /* A side-effect less read */
    fn read_mut(&mut self, addr: u16) -> u8;   /* A read with side-effects */
    
    // Convenience functions to read an address word
    fn read_16(&self, addr: u16) -> u16;
    fn read_16_mut(&mut self, addr: u16) -> u16;
}

// Intrusive read
impl MemoryRead for CPUMemory<'_> {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                /* Internal RAM */
                self.internal_ram[(addr & 0x07FF) as usize]
            }
            0x2000..=0x3FFF => {
                panic!("Attempted read of address with side-affect from observer.")
            }
            0x4000..=0x4017 => {
                /* I/O registers - defer to MemoryRead */
                self.io_registers[(addr - 0x4000) as usize]
            }
            0x4018..=0x401F => {
                /* CPU test mode registers */
                0
            }
            0x4020..=0xFFFF => {
                /* Mapped - may have side-effects for mapper */
                self.mapper.read(addr)
            }
        }
    }

    fn read_mut(&mut self, addr: u16) -> u8 {
        match addr {
            0x0000..=0x1FFF => {
                /* Internal RAM */
                self.internal_ram[(addr & 0x07FF) as usize]
            }
            0x2000..=0x3FFF => {
                self.ppu_registers.as_mut().unwrap().borrow_mut().ppu_register_read(0x2000 + (addr & 0x7))
            }
            0x4000..=0x4017 => {
                /* I/O registers - defer to MemoryRead */
                let data: u8;

                if addr == 0x4016 { /* JOY1 */
                    // Return and shift the controller shift register
                    data = *self.joy1_in.borrow() & 0x1;
                    if !self.joy_freeze {
                        *self.joy1_in.borrow_mut() >>= 1;
                    }
                } else { data = 0; }
                data
            }
            0x4018..=0x401F => {
                /* CPU test mode registers */
                0
            }
            0x4020..=0xFFFF => {
                /* Mapped - may have side-effects for mapper */
                self.mapper.read(addr) //  TODO: Make this a read_mut
            }
        }
        
    }

    fn read_16(&self, addr: u16) -> u16 {
        ((self.read(addr)) as u16) | (((self.read(addr + 1)) as u16) << 8)
    }

    fn read_16_mut(&mut self, addr: u16) -> u16 {
        ((self.read_mut(addr)) as u16) | (((self.read_mut(addr + 1)) as u16) << 8)
    }
}

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

pub struct CPUMemory<'a> {
    pub internal_ram: [u8; 0x0800],
    pub io_registers: IORegisters,
    pub mapper: Box<dyn Mapper<u8, ()>>,
    pub ppu_registers: Option<Rc<RefCell<NESPpu<'a>>>>,
    pub joy1_in: &'a RefCell<u8>,
    pub joy_freeze: bool,
}

impl<'a> CPUMemory<'a> {
    pub fn write(&mut self, addr: u16, data: u8) -> Result<(), String> {
        /* Internal RAM */
        if (addr & 0xF000) < 0x2000 {
            self.internal_ram[(addr & 0x07FF) as usize] = data;
        }

        /* PPU control registers */
        /* TODO - in reality these are PPU mapped and take effect */
        if (addr & 0xF000) == 0x2000 || (addr & 0xF000) == 0x3000 {
            self.ppu_registers.as_mut().unwrap().borrow_mut().ppu_register_write(0x2000 + (addr & 0x7), data);
        }

        /* APU and I/O */
        if (addr >= 0x4000) && (addr <= 0x4017) {
            if addr == 0x4016 {
                if data & 0x1 == 0x1 {
                    // Reload the controller(s) shift registers
                    self.joy_freeze = true;
                    self.io_registers[0x16] = *self.joy1_in.borrow(); 
                } else {
                    // Unfreeze the shift registers to allow program to query buttons
                    self.joy_freeze = false;
                }
            }
            self.io_registers[(addr - 0x4000) as usize] = data;
        }

        /* CPU test mode registers */
        if (addr >= 0x4018) && (addr <= 0x401F) {
            // Nothing
        }

        /* Any address 0x4020 - 0xFFFF is handled by a mapper */
        if (addr >= 0x4020) && (addr <= 0xFFFF) {
            return self.mapper.write(addr, data);
        }

        Ok(())
    }
}