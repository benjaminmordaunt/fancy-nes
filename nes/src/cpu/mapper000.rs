use crate::Mirroring;

use super::mapper::Mapper;

// For NROM-128, $C000-$FFFF mirrors $8000-$BFFF,
// so we need to specify which size we want (16K / 32K)
// Note that for all mappers, the hardwired
// mirroring is handled separately.

// 8KiB of PRG RAM is provided to fill 0x6000 - 0x7FFF window
// Most games shouldn't depend on mirrored addresses, so let's
// hope for the best!

pub struct Mapper000 {
    prg_rom: Vec<u8>,
    chr_rom: Vec<u8>,  /* The CHR (character) ROM, static graphics tile data */
    prg_ram: [u8; 8192],

    mirroring: Mirroring,
}

impl Mapper for Mapper000 {
    fn read(&self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                return self.prg_ram[addr as usize - 0x6000];
            }
            0x8000..=0xBFFF => {
                return self.prg_rom[addr as usize - 0x8000];
            }
            0xC000..=0xFFFF => {
                if self.prg_rom.capacity() == 16384 {
                    return self.prg_rom[addr as usize - 0xC000];
                } else {
                    return self.prg_rom[addr as usize - 0x8000];
                }
            }
            _ => { 0 }
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x6000..=0x7FFF => {
                self.prg_ram[addr as usize - 0x6000] = data;
            }
            _ => {}
        }
    }

    fn load_prg_rom(&mut self, rom: &Vec<u8>) {
        assert!(match rom.capacity() {
            16384 => true,
            32768 => true,
            _ => false
        });

        self.prg_rom = rom.clone();
    }

    fn load_chr_rom(&mut self, rom: &Vec<u8>) {
        self.chr_rom = rom.clone();
    }

    fn read_ppu(&self, mut addr: u16) -> u16 {
        match addr {
            0x0000..=0x1FFF => {
                self.chr_rom[addr as usize] as u16
            }
            0x2000..=0x2FFF => {
                match self.mirroring {
                    Mirroring::Horizontal => {
                        addr &= !(1 << 10);
                    }
                    Mirroring::Vertical => {
                        addr &= !(1 << 11);
                    }
                    _ => { unreachable!() }
                }
                0x1000 | (addr - 0x2000)
            }
            0x3000..=0x3EFF => {
                0
            }
            _ => { unreachable!() }
        }
    }

    fn write_ppu(&mut self, mut addr: u16, data: u8) -> u16 {
        match addr {
            0x0000..=0x1FFF => {
                self.chr_rom[addr as usize] = data;
                0
            }
            0x2000..=0x2FFF => {
                match self.mirroring {
                    Mirroring::Horizontal => {
                        addr &= !(1 << 10);
                    }
                    Mirroring::Vertical => {
                        addr &= !(1 << 11);
                    }
                    _ => { unreachable!() }
                } 
                0x1000 | (addr - 0x2000)
            }
            0x3000..=0x3EFF => {
                0
            }
            _ => { unreachable!() }
        }
    }
}

impl Mapper000 {
    pub fn new(mirroring: Mirroring) -> Self {
        Self {
            prg_rom: Vec::new(),
            prg_ram: [0; 8192],

            chr_rom: vec![],
            mirroring
        }
    }
}

