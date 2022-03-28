use super::mapper::Mapper;

// For NROM-128, $C000-$FFFF mirrors $8000-$BFFF,
// so we need to specify which size we want (16K / 32K)
// Note that for all mappers, the hardwired
// mirroring is handled separately.

// 8KiB of PRG RAM is provided to fill 0x6000 - 0x7FFF window
// Most games shouldn't depend on mirrored addresses, so let's
// hope for the best!

pub struct Mapper000<const PRG_ROM_SIZE: usize> {
    prg_rom: [u8; PRG_ROM_SIZE],
    prg_ram: [u8; 8192],
}

impl<const PRG_ROM_SIZE: usize> Mapper for Mapper000<PRG_ROM_SIZE> {
    fn read(&mut self, addr: u16) -> u8 {
        match addr {
            0x6000..=0x7FFF => {
                return self.prg_ram[addr as usize - 0x6000];
            }
            0x8000..=0xBFFF => {
                return self.prg_rom[addr as usize - 0x8000];
            }
            0xC000..=0xFFFF => {
                if PRG_ROM_SIZE == 16384 {
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
}

