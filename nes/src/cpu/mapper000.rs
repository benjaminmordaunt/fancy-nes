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
    prg_ram: [u8; 8192],
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
}

impl Mapper000 {
    pub fn new() -> Self {
        Self {
            prg_rom: Vec::new(),
            prg_ram: [0; 8192],
        }
    }
}

