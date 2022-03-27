/// The PPU (picture processing unit) generates 2D graphics and
/// is effectively a separate processor (Ricoh 2C02 on NTSC units).
/// While untrue for the PAL NES (TODO), its clock is approximated as
/// 3 PPU "dots" = 1 CPU cycle. Here is some important information (excl. Dendy):
/// TODO
use bitflags::bitflags;

mod PPUAddress {
    const PPUCTRL: u16   = 0x2000;
    const PPUMASK: u16   = 0x2001;
    const PPUSTATUS: u16 = 0x2002;
    const OAMADDR: u16   = 0x2003;
    const OAMDATA: u16   = 0x2004;
    const PPUSCROLL: u16 = 0x2005;
    const PPUADDR: u16   = 0x2006;
    const PPUDATA: u16   = 0x2007;
    const OAMDMA: u16    = 0x4014;
}

bitflags! {
    struct PPUCTRL: u8 {
        const BASE_NAMETABLE_ADDR_LO = 0b00000001;
        const BASE_NAMETABLE_ADDR_HI = 0b00000010;
        const VRAM_INCREMENT         = 0b00000100;
        const SPRITE_TABLE_ADDR      = 0b00001000;
        const BACKGROUND_TABLE_ADDR  = 0b00010000;
        const SPRITE_SIZE            = 0b00100000;
        const PPU_ORIENTATION        = 0b01000000;
        const NMI_ENABLED            = 0b10000000;
    }
}

bitflags! {
    struct PPUMASK: u8 {
        const GREYSCALE       = 0b00000001;
        const LEFT_BACKGROUND = 0b00000010;
        const LEFT_SPRITES    = 0b00000100;
        const BACKGROUND      = 0b00001000;
        const SPRITES         = 0b00010000;
        const EMPH_RED        = 0b00100000;
        const EMPH_GREEN      = 0b01000000;
        const EMPH_BLUE       = 0b10000000;
    }
}

bitflags! {
    struct PPUSTATUS: u8 {
        const SPRITE_OVERFLOW  = 0b00100000;
        const SPRITE_ZERO_HIT  = 0b01000000;
        const VBLANK           = 0b10000000;
    }
}

struct NESPPU {
    chr_rom: Vec<u8>,  /* The CHR (character) ROM, static graphics tile data */
    /* Palette memory map:
        0      - universal background colour     \
        1..3   - background palette 0            /`--- (bg 0 selected)
        4      - (aliases to 0*+)                \
        5..7   - background palette 1            /`--- (bg 1 selected)
        8      - (aliases to 0*+)                \
        9..B   - background palette 2            /`--- (bg 2 selected)
        C      - (aliases to 0*+)                \
        D..F   - background palette 3            /`--- (bg 3 selected)
        10     - (aliases to 0*)                 \
        11..13 - sprite palette 0                /`--- (sp 0 selected)
        14     - (aliases to 0*)                 \
        15..17 - sprite palette 1                /`--- (sp 1 selected)
        18     - (aliases to 0*)                 \
        19..1B - sprite palette 2                /`--- (sp 2 selected)
        1C     - (aliases to 0*)                 \
        1D..1F - sprite palette 3                /`--- (sp 3 selected)

        * aliases to the universal background colour are also writable!
        + can actually contain unique data - xx10,xx14,xx1C are then aliases of
          these. In this emulator, these all map to xx00, however 
    */
    palette: [u8; 32],
    vram: [u8; 2048],   /* 2KB of RAM inside the NES dedicated to the PPU     */
    oam: [u8; 256],     /* CPU can manipulate via memory-mapped DMA registers */

    write_toggle: bool, /* The latch shared by $2005, $2006 to distinguish 
                          between first and second writes. */
    vram_v: u16,        /* Current VRAM address */
    vram_t: u16,        /* Staging area for VRAM address copy */
    vram_x: u16,        /* Fine X scroll (actually 3 bits wide) */
}

impl NESPPU {
    /// Fetches the address of the tile and attribute data for a given VRAM access
    fn tile_attr_from_vram_addr(addr: u16) -> (u16, u16) {
        ((0x2000 | (addr & 0x0FFF)),
         (0x23C0 | (addr & 0x0C00) | ((addr >> 4) & 0x38) | ((addr >> 2) & 0x07)))
    }

    fn ppu_register_write(&mut self, addr: u16, data: u8) {
        use PPUAddress::*;

        match addr {
        PPUCTRL => {
            // Populate lo-nybble of high byte of base nametable address
            self.vram_t = (self.vram_t & 0xF3FF) | ((data & 0x3) << 10);
        }
        PPUSCROLL => {
            if !self.write_toggle {
                self.vram_x = data & 0x7;
                self.vram_t = (self.vram_t & 0xFFE0) | ((data >> 0x3) & 0x1F);
            } else {
                self.vram_t = (self.vram_t & 0x8C1F) | ((data & 0x3) << 12)
                            | ((data & 0x38) << 2) | ((data & 0xC0) << 2);
            }
            self.write_toggle = !self.write_toggle;
        }
        PPUADDR => {
            if !self.write_toggle {
                self.vram_t = (self.vram_t & 0xFF) | ((data & 0x3F) << 8);
            } else {
                self.vram_t = (self.vram_t & 0xFF00) | data;
                self.vram_v = self.vram_t;
            }
            self.write_toggle = !self.write_toggle;
        }
        }
    }

    fn ppu_register_read(&mut self, addr: u16) -> u8 {
        use PPUAddress::*;

        match addr {
        PPUSTATUS => {
            self.write_toggle = false;
        }
        }
    }
}