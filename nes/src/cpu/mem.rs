use super::mapper::Mapper;
struct PPURegisters {
    
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

pub struct CPUMemory {
    internal_ram: [u8; 0x0800],
    ppu_registers: [u8; 0x0008],
    io_registers: IORegisters,
    cartridge_mapper: dyn Mapper,
}

impl CPUMemory {
    pub fn read(&self, addr: u16) -> u8 {
        /* Internal RAM */
        if (addr & 0xF000) < 0x2000 {
            return self.internal_ram[(addr & 0x07FF) as usize];
        }

        /* PPU control registers */
        if (addr & 0xF000) == 0x2000 || (addr & 0xF000) == 0x3000 {
            return self.ppu_registers[(addr & 0x0007) as usize];
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

    pub fn read_16(&self, addr: u16) -> u16 {
        self.read(addr) as u16
        + (self.read(addr + 1) as u16) << 8
    }

    pub fn write(&self, addr: u16, data: u8) {
        todo!();
    }
}