use std::{cell::RefCell, rc::Rc};

/// The PPU (picture processing unit) generates 2D graphics and
/// is effectively a separate processor (Ricoh 2C02 on NTSC units).
/// While untrue for the PAL NES (TODO), its clock is approximated as
/// 3 PPU "dots" = 1 CPU cycle. Here is some important information (excl. Dendy):
/// TODO
use bitflags::bitflags;

use crate::cpu::NESCpu;

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

        const RENDERING = Self::BACKGROUND.bits | Self::SPRITES.bits;
    }
}

bitflags! {
    struct PPUSTATUS: u8 {
        const SPRITE_OVERFLOW  = 0b00100000;
        const SPRITE_ZERO_HIT  = 0b01000000;
        const VBLANK           = 0b10000000;
    }
}

pub struct NESPPU {
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
    pub palette: [u8; 32],
    vram: [u8; 2048],   /* 2KB of RAM inside the NES dedicated to the PPU     */
    oam: [u8; 256],     /* CPU can manipulate via memory-mapped DMA registers */

    write_toggle: bool, /* The latch shared by $2005, $2006 to distinguish 
                          between first and second writes. */
    scanline: u16,      /* The next scanline to be rendered (0-261 NTSC) */

    /* Note that the vram_v and vram_t are organised as follows:
        yyy NN YYYYY XXXXX
        ||| || ||||| +++++-- course X scroll
        ||| || +++++-------- course Y scroll
        ||| ++-------------- nametable select
        +++----------------- fine Y scroll
    */
    vram_v: u16,        /* Current VRAM address */
    vram_t: u16,        /* Staging area for VRAM address copy */
    vram_x: u16,        /* Fine X scroll (actually 3 bits wide) */

    /* PPU registers */
    ppu_ctrl: PPUCTRL,
    ppu_mask: PPUMASK,
    ppu_status: PPUSTATUS,
    /* End PPU registers */

    addr_data_bus: u16,  /* The PPU uses the same bus for addr and data to save pins */
    tick: u16,           /* The tick on the current scanline (0-indexed) */

    bg_pattern_shift_reg_hi: u16,  /* Background pattern table shift registers */
    bg_pattern_shift_reg_lo: u16,

    bg_pattern_next_hi: u8,
    bg_pattern_next_lo: u8,
     
    bg_attribute_shift_reg_hi: u8, /* Background palette attributes shift registers */
    bg_attribute_shift_reg_lo: u8,

    bg_attribute_next_hi: u8,
    bg_attribute_next_lo: u8,

    /* Bit planes from the pattern table */
    bg_next_pattern_lsb: u8,
    bg_next_pattern_msb: u8,

    /* Tile index into the nametable and attribute for next tile */
    bg_next_tile: u8,
    bg_next_attr: u8,

    cpu: Rc<RefCell<NESCpu>>,             /* A ref to CPU which lives at least as long as the PPU! (for interrupts) */
} 

impl NESPPU {
    pub fn new(cpu: Rc<RefCell<NESCpu>>) -> Self {
        Self {
            chr_rom: vec![],
            palette: [0; 32],
            vram: [0; 2048],
            oam: [0; 256],
            write_toggle: false,
            scanline: 261,
            vram_v: 0,
            vram_t: 0,
            vram_x: 0,
            ppu_ctrl: PPUCTRL::from_bits_truncate(0x00),
            ppu_mask: PPUMASK::from_bits_truncate(0x00),
            ppu_status: PPUSTATUS::from_bits_truncate(0x00),
            addr_data_bus: 0,
            tick: 0,

            bg_pattern_shift_reg_lo: 0,
            bg_pattern_shift_reg_hi: 0,
            bg_attribute_shift_reg_lo: 0,
            bg_attribute_shift_reg_hi: 0,

            bg_pattern_next_hi: 0,
            bg_pattern_next_lo: 0,

            bg_attribute_next_hi: 0,
            bg_attribute_next_lo: 0,

            bg_next_attr: 0,
            bg_next_tile: 0,

            bg_next_pattern_lsb: 0,
            bg_next_pattern_msb: 0,

            cpu
        }
    }

    pub fn load_chr_rom(&mut self, rom: &Vec<u8>) {
        self.chr_rom = rom.clone();
    }

    /// Fetches the address of the tile and attribute data for a given VRAM access
    fn tile_attr_from_vram_addr(addr: u16) -> (u16, u16) {
        ((0x2000 | (addr & 0x0FFF)),
         (0x23C0 | (addr & 0x0C00) | ((addr >> 4) & 0x38) | ((addr >> 2) & 0x07)))
    }

    fn ppu_register_write(&mut self, addr: u16, data: u8) {

        match addr {
        PPUCTRL => {
            // Populate lo-nybble of high byte of base nametable address
            self.vram_t = (self.vram_t & 0xF3FF) | ((data as u16 & 0x3) << 10);
        }
        PPUSCROLL => {
            if !self.write_toggle {
                self.vram_x = data as u16 & 0x7;
                self.vram_t = (self.vram_t & 0xFFE0) | ((data as u16 >> 0x3) & 0x1F);
            } else {
                self.vram_t = (self.vram_t & 0x8C1F) | ((data as u16 & 0x3) << 12)
                            | ((data as u16 & 0x38) << 2) | ((data as u16 & 0xC0) << 2);
            }
            self.write_toggle = !self.write_toggle;
        }
        PPUADDR => {
            if !self.write_toggle {
                self.vram_t = (self.vram_t & 0xFF) | ((data as u16 & 0x3F) << 8);
            } else {
                self.vram_t = (self.vram_t & 0xFF00) | data as u16;
                self.vram_v = self.vram_t;
            }
            self.write_toggle = !self.write_toggle;
        }
        }
    }

    fn ppu_register_read(&mut self, addr: u16) -> u8 {
        let data: u8;

        match addr {
        PPUSTATUS => {
            data = self.ppu_status.bits();
            self.ppu_status.remove(PPUSTATUS::VBLANK);
            self.write_toggle = false;
        }
        _ => { todo!() }
        }

        data
    }
    
    /// Handles rendering behaviour of the PPU from a "high level"
    /// i.e. depends only on the current scanline (0-261)
    fn ppu_do_scanline(&mut self) {
        // If we're not in VBLANK and rendering, do data address/fetching
        if !self.ppu_status.contains(PPUSTATUS::VBLANK) && self.ppu_mask.contains(PPUMASK::RENDERING) {
            if self.tick % 2 == 0 {
                // Address
                self.addr_data_bus = self.vram_v;
            } else {
                // Readback
                self.addr_data_bus = (self.addr_data_bus & 0xFF00) 
                    | self.vram[self.addr_data_bus as usize] as u16;
            }
        }

        match self.scanline {
        // Visible scanlines
        0..=239 => {
            
        }
        241 => {
            if self.tick == 1 {
                // If NMI enabled in ppu_ctrl, raise an interrupt to the CPU
                if self.ppu_ctrl.contains(PPUCTRL::NMI_ENABLED) {
                    self.cpu.borrow_mut().nmi();
                }
            }
        }
        _ => { todo!() }
        }
    }

    /// (NTSC) 3 of these happen per CPU tick.
    /// Events within the PPU are "batched" together if at all possible.
    /// That is to say, if self.tcount < <Event's tick> <= (self.tcount + count),
    /// that event is executed, otherwise tcount is incremented by count
    /// and we move on with life.
    pub fn ppu_tick(&mut self, count: usize) {
        match self.scanline {
            // All "rendering" scanlines - those which make standard PPU memory accesses.
            0..=239 | 261 => {
                // Pre-render scanline
                if self.scanline == 261 {
                    // Clear the PPU's status
                    self.ppu_status = PPUSTATUS::from_bits_truncate(0);
                }

                match (self.tick - 1) % 8 {
                    0 => {
                        // Load the background shift registers with pattern table data
                        self.bg_pattern_shift_reg_hi = (self.bg_pattern_shift_reg_hi & 0x00FF) | (self.bg_pattern_next_hi as u16) << 8;
                        self.bg_pattern_shift_reg_lo = (self.bg_pattern_shift_reg_lo & 0x00FF) | (self.bg_pattern_next_lo as u16) << 8;

                        // Load the attribute shift registers with an expanded (8x1 slither) attribute value
                        self.bg_attribute_shift_reg_hi = if self.bg_attribute_next_hi & 1 == 1 { 0xFF } else { 0x00 };
                        self.bg_attribute_shift_reg_lo = if self.bg_attribute_next_lo & 1 == 1 { 0xFF } else { 0x00 };

                        self.bg_next_tile = self.read(&NESPPU::tile_attr_from_vram_addr(self.vram_v).0);
                    }
                    2 => {
                        self.bg_next_attr = self.read(&NESPPU::tile_attr_from_vram_addr(self.vram_v).1);
                    }
                    4 => {
                        // Get the lsb bit plane from the pattern table for the next tile
                        self.bg_next_pattern_lsb = self.read(
                            (self.ppu_ctrl.contains(PPUCTRL::BACKGROUND_TABLE_ADDR) as u16) << 12
                        |   (self.bg_next_tile as u16) << 4
                        |   ((self.vram_v & 0x7000) << 12)); 
                    }
                    6 => {
                        // Get the msb bit plane from the pattern table for the next tile (+8 offset from LSB)
                        self.bg_next_pattern_lsb = self.read(
                            (self.ppu_ctrl.contains(PPUCTRL::BACKGROUND_TABLE_ADDR) as u16) << 12
                        |   (self.bg_next_tile as u16) << 4
                        |   ((self.vram_v & 0x7000) << 12) + 8);
                    }
                    7 => {
                        // Scroll horizontally (algorithm taken from NESDEV)
                        if self.vram_v & 0x001F == 31 { // Are we at the end of a nametable?
                            self.vram_v &= !0x001F;     // Reset course X to 0
                            self.vram_v ^= 0x0400;      // Switch the horizontal nametable
                        } else {
                            self.vram_v += 1; // Increment as usual :-)
                        }
                    }
                }

                if self.tick == 256 {
                    // When we reach the end of a scanline, increment the fine Y-scroll, then course vertical scroll.
                    // Again, this algorithm is lovingly taken from NESDEV.
                    if self.vram_v & 0x7000 != 0x7000 {
                        self.vram_v += 0x1000; // Standard fine-Y increment
                    } else {
                        self.vram_v &= !0x7000;                       // Reset fine-Y to 0
                        let mut y = (self.vram_v & 0x03E0) >> 5; // Fine-y = course-y
                        if y == 29 {
                            y = 0;
                            self.vram_v ^= 0x0800;  // Switch the vertical nametable
                        } else if y == 31 {
                            y = 0;                  // Reset course Y, but don't switch nametable
                        } else {
                            y += 1;                 // Increment course-Y
                        }
                        self.vram_v = (self.vram_v & !0x03E0) | (y << 5);
                    }
                }

                if self.tick == 257 {
                    // If rendering is enabled, transfer the X-affiliated parts of vram_t to vram_v.
                    if self.ppu_mask.contains(PPUMASK::RENDERING) {
                        self.vram_v = (self.vram_v & !0x41F) | (self.vram_t & 0x41F);
                    }
                }

                if self.scanline == 261 && self.tick >= 280 && self.tick <= 304 {
                    // End of the VBLANK period, copy the vertical bits from vram_t to vram_v.
                    if self.ppu_mask.contains(PPUMASK::RENDERING) {
                        self.vram_v = (self.vram_v & !0x7BE0) | (self.vram_t & 0x7BE0);
                    }
                }
            }
            241..=260 => {
                if self.scanline == 241 && self.tick == 1 {
                    self.ppu_status.insert(PPUSTATUS::VBLANK);
                    self.cpu.borrow_mut().nmi();
                }
            }
        }
    }

    pub fn render<F: FnMut(&[u8; 61440]) -> ()>(&self, mut f: F) {
        let mut result: [u8; 61440] = [0; 61440];

        let pattern_bank = (self.ppu_ctrl.contains(PPUCTRL::BACKGROUND_TABLE_ADDR) as u16) << 3;

        for i in 0..0x03C0 {
            let tile = self.vram[i] as u16; // Fetch the nametable byte from cartridge-mapped CHR ROM/RAM
            let tile_x = tile % 32;
            let tile_y = tile / 32;
            let tile = &self.chr_rom[(pattern_bank + tile * 16) as usize..=(pattern_bank + tile * 16 + 15) as usize];

            for y in 0..8 { // The row number within a tile
                let mut lower = tile[y]; // The colour index bit planes
                let mut upper = tile[y + 8];

                // The 8 pixels in the x-direction (add to result in top-left to bottom-right order)
                for x in (0..8).rev() {
                    let value = (lower & 1) | (upper & 1) << 1;
                    lower >>= 1;
                    upper >>= 1;
                    result[(tile_y as usize + y) * 64 + (tile_x as usize + x)] = value;
                }
            }
        }

        f(&result);
    }
}