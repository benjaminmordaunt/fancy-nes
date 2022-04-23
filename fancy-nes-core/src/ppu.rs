/* Copyright (c) 2022 Benjamin John Mordaunt */
/* See LICENSE */

use std::process::exit;
use std::{cell::RefCell, rc::Rc};

/// The PPU (picture processing unit) generates 2D graphics and
/// is effectively a separate processor (Ricoh 2C02 on NTSC units).
/// While untrue for the PAL NES (TODO), its clock is approximated as
/// 3 PPU "dots" = 1 CPU cycle. Here is some important information (excl. Dendy):
/// TODO
use bitflags::bitflags;

use crate::Mirroring;
use crate::cpu::NESCpu;
use crate::cpu::mapper::Mapper;
use crate::cpu::mapper000::PPUMapper000;
mod PPUAddress {
    pub const PPUCTRL: u16   = 0x2000;
    pub const PPUMASK: u16   = 0x2001;
    pub const PPUSTATUS: u16 = 0x2002;
    pub const OAMADDR: u16   = 0x2003;
    pub const OAMDATA: u16   = 0x2004;
    pub const PPUSCROLL: u16 = 0x2005;
    pub const PPUADDR: u16   = 0x2006;
    pub const PPUDATA: u16   = 0x2007;
    pub const OAMDMA: u16    = 0x4014;
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

pub struct NESPpu<'a> {
    /* Palette memory map:
        0      - universal background colour     \
        1..3   - background palette 0            /`--- (bg 0 selected)
        4      - (aliases to 0*+)                \
        5..7   - background palette 1            /`--- (bg 1 selected)
        8      - (aliases to 0*+)                \
        9..B   - background palette 2            /`--- (bg 2 selected)
        C      - (aliases to 0*+)                \
        D..F   - bacckground palette 3           /`--- (bg 3 selected)
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

    // pOAM public for OAMDMA
    pub poam: [u8; 64*4],   /* Primary OAM can be DMA'd using OAMDATA */
    soam: [u8; 8*4],    /* CPU can manipulate via memory-mapped DMA registers */
                        /* Secondary OAM - stores 8 sprites for current scanline */
                        

    write_toggle: bool, /* The latch shared by $2005, $2006 to distinguish 
                          between first and second writes. */
    pub scanline: u16,      /* The next scanline to be rendered (0-261 NTSC) */

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
    oam_address: u8,
    oam_data: u8,
    /* End PPU registers */

    pub tick: u16,           /* The tick on the current scanline (0-indexed) */

    bg_pattern_shift_reg_hi: u16,  /* Background pattern table shift registers */
    bg_pattern_shift_reg_lo: u16,

    /* Bit planes from the pattern table */
    bg_pattern_next_hi: u8,
    bg_pattern_next_lo: u8,
     
    bg_attribute_shift_reg_hi: u16, /* Background palette attributes shift registers */
    bg_attribute_shift_reg_lo: u16,

    bg_attribute_next_hi: u8,
    bg_attribute_next_lo: u8,

    /* Tile index into the nametable and attribute for next tile */
    bg_next_tile: u8,
    bg_next_attr: u8,

    // PPUDATA is buffered by one CPU access
    data_bus_next: u8,

    // PPU sprite evaluation
    poam_data: u8,
    poam_sprite_index: usize,
    poam_sprite_byte_index: usize,
    soam_next_open_slot: usize,
    sprite_evaluation_substage: usize,
    sprite_evaluation_write_rest: bool,

    // PPU sprite rendering
    sprite_render_oam_y_coord: u8,
    sprite_render_oam_tile_number: u8,
    sprite_render_oam_attribute: u8,
    sprite_render_oam_x_coord: u8,

    // Latches and counters for sprite rendering
    sprite_pattern_shift_reg_hi: [u8; 8],
    sprite_pattern_shift_reg_lo: [u8; 8],
    sprite_attribute_latch: [u8; 8],
    sprite_x_position_counter: [u8; 8],

    cpu: Rc<RefCell<NESCpu<'a>>>,             /* A ref to CPU which lives at least as long as the PPU! (for interrupts) */

    pub frame: [u8; 61440],  /* A frame, to be rendered when frame_complete is signalled */
    pub frame_ready: bool,

    pub mapper: Box<dyn Mapper<u16, u16>>,
} 

impl<'a> NESPpu<'a> {
    pub fn new(mapper_id: usize, cpu: Rc<RefCell<NESCpu<'a>>>, mirroring: Mirroring) -> Self {
        Self {
            palette: [0; 32],
            vram: [0; 2048],
            poam: [0; 64*4],
            soam: [0; 8*4],
            write_toggle: false,
            scanline: 261,
            vram_v: 0,
            vram_t: 0,
            vram_x: 0,
            ppu_ctrl: PPUCTRL::from_bits_truncate(0x00),
            ppu_mask: PPUMASK::from_bits_truncate(0x00),
            ppu_status: PPUSTATUS::from_bits_truncate(0x00),
            oam_address: 0,
            oam_data: 0,
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

            data_bus_next: 0,

            poam_data: 0xFF,
            poam_sprite_index: 0,
            poam_sprite_byte_index: 0,
            soam_next_open_slot: 0,
            sprite_evaluation_substage: 1,
            sprite_evaluation_write_rest: false,

            sprite_render_oam_y_coord: 0,
            sprite_render_oam_tile_number: 0,
            sprite_render_oam_attribute: 0,
            sprite_render_oam_x_coord: 0,

            sprite_pattern_shift_reg_hi: [0; 8],
            sprite_pattern_shift_reg_lo: [08; 8],
            sprite_attribute_latch: [0; 8],
            sprite_x_position_counter: [0; 8],

            frame: [0; 61440],
            frame_ready: false,
            cpu,

            mapper: Box::new(
                match mapper_id {
                    0 => { PPUMapper000::new(mirroring) }
                    _ => { unimplemented!() }
                }
            )
        }
    }

    pub fn read(&self, mut addr: u16) -> u8 {
        match addr {
            // Remappable addresses by the mapper - might come straight back to internal VRAM if mapped that way!
            // If the mapper returns a word starting with 0x1***, treat *** as an index into PPU RAM.
            0x0000..=0x3EFF => {
                let word: u16;
                word = self.mapper.read(addr);

                if word & 0x1000 > 0 {
                    self.vram[word as usize & 0x0FFF]
                } else {
                    (word & 0xFF) as u8
                }
            }
            0x3F00..=0x3FFF => {
                // Alias sprite clear accesses to the background clear accesses.
                match addr {
                    0x3F10 | 0x3F14 | 0x3F18 | 0x3F1C => { addr -= 0x10; }
                    _ => {}
                }
                self.palette[addr as usize & 0x1F] & (if self.ppu_mask.contains(PPUMASK::GREYSCALE) { 0x30 } else { 0x3F })
            }
            _ => { unreachable!() }
        }
    }

    fn write(&mut self, addr: u16, data: u8) {
        match addr {
            0x0000..=0x3EFF => {
                let word: u16;
                word = self.mapper.write(addr, data).unwrap();

                if word & 0x1000 > 0 {
                    self.vram[word as usize & 0x0FFF] = data;
                } else {
                    // Written by mapper
                }
            }
            0x3F00..=0x3FFF => {
                self.palette[(addr & 0x1F) as usize] = data;
            }
            _ => { unreachable!() }
        }
    }

    /// Fetches the address of the tile and attribute data for a given VRAM access
    fn tile_attr_from_vram_addr(addr: u16) -> (u16, u16) {
        ((0x2000 | (addr & 0x0FFF)),
         (0x23C0 | (addr & 0x0C00) | ((addr >> 4) & 0x38) | ((addr >> 2) & 0x07)))
    }

    // Interpreted in terms of the CPU's address space
    pub fn ppu_register_write(&mut self, addr: u16, data: u8) {
        match addr {
        PPUAddress::PPUCTRL => {
            // Populate lo-nybble of high byte of base nametable address
            self.vram_t = (self.vram_t & 0xF3FF) | ((data as u16 & 0x3) << 10);

            self.ppu_ctrl = PPUCTRL::from_bits_truncate(data);
        }
        PPUAddress::PPUMASK => {
            // TODO: Implement background and sprite hiding in the leftmost 8 pixels + colour emphasis
            self.ppu_mask = PPUMASK::from_bits_truncate(data);
        }
        PPUAddress::PPUSCROLL => {
            if !self.write_toggle {
                self.vram_x = data as u16 & 0x7;
                self.vram_t = (self.vram_t & 0xFFE0) | ((data as u16 >> 0x3) & 0x1F);
            } else {
                self.vram_t = (self.vram_t & 0x8C1F) | ((data as u16 & 0x3) << 12)
                            | ((data as u16 & 0x38) << 2) | ((data as u16 & 0xC0) << 2);
            }
            self.write_toggle = !self.write_toggle;
        }
        PPUAddress::PPUADDR => {
            if !self.write_toggle {
                self.vram_t = (self.vram_t & 0xFF) | ((data as u16 & 0x3F) << 8);
            } else {
                self.vram_t = (self.vram_t & 0xFF00) | data as u16;
                self.vram_v = self.vram_t;
            }
            self.write_toggle = !self.write_toggle;
        }
        PPUAddress::PPUDATA => {
            // Just immediately write the data
            self.write(self.vram_v & 0x3FFF, data);

            // Perform VRAM addr increment
            let increment = if self.ppu_ctrl.contains(PPUCTRL::VRAM_INCREMENT) { 32 } else { 1 };
            self.vram_v += increment;
        }
        PPUAddress::OAMADDR => {
            // In reality, sprite evaluation starts wherever this is set.
            // This has implications for alignment enforcements of oam_address.
            // TODO, have sprite evaluation start from here.
            self.oam_address = data;
        }
        PPUAddress::OAMDATA => {
            self.poam[self.oam_address as usize] = data;
            self.oam_address += 1;
        }
        _ => { panic!("{:#X}", addr) }
        }
    }

    // Addresses interpreted in terms of the CPU's address space
    // Reads from these registers typically exhibit side effects (hence the mut ref)
    pub fn ppu_register_read(&mut self, addr: u16) -> u8 {
        let data: u8;

        match addr {
        PPUAddress::PPUSTATUS => {
            data = self.ppu_status.bits();
            self.ppu_status.remove(PPUSTATUS::VBLANK);
            self.write_toggle = false;
        }
        PPUAddress::PPUDATA => {
            if self.vram_v < 0x03F00 {
                // Update the internal buffer
                data = self.data_bus_next;
                self.data_bus_next = self.read(self.vram_v & 0x3FFF);
            } else {
                // Otherwise, we get palette data via combinatorial logic
                data = self.read(addr & 0x3FFF);
            }

            // Perform VRAM addr increment
            let increment = if self.ppu_ctrl.contains(PPUCTRL::VRAM_INCREMENT) { 32 } else { 1 };
            self.vram_v += increment;
        }
        _ => { panic!("{:X}", addr) }
        }
        data
    }

    /// (NTSC) 3 of these happen per CPU tick.
    /// Events within the PPU are "batched" together if at all possible.
    /// That is to say, if self.tcount < <Event's tick> <= (self.tcount + count),
    /// that event is executed, otherwise tcount is incremented by count
    /// and we move on with life.
    pub fn ppu_tick(&mut self, count: usize) {
        for _ in 0..count {
            match self.scanline {
                // All "rendering" scanlines - those which make standard PPU memory accesses.
                0..=239 | 261 => {
                    // Idle-skip on first scanline (picture crispness - apparently)
                    if self.scanline == 0 && self.tick == 0 {
                        self.tick = 1;
                    }

                    // Pre-render scanline
                    if self.scanline == 261 && self.tick == 1 {
                        // Clear the PPU's status
                        self.ppu_status = PPUSTATUS::from_bits_truncate(0);
                    }

                    if matches!(self.tick, 2..=258 | 321..=336) {
                        if self.ppu_mask.contains(PPUMASK::BACKGROUND) {
                            self.bg_attribute_shift_reg_hi <<= 1;
                            self.bg_attribute_shift_reg_lo <<= 1;

                            self.bg_pattern_shift_reg_hi <<= 1;
                            self.bg_pattern_shift_reg_lo <<= 1;
                        }

                        if self.ppu_mask.contains(PPUMASK::SPRITES) && self.tick < 258 {
                            for sprite in 0usize..8usize {
                                if self.sprite_x_position_counter[sprite] > 0 {
                                    self.sprite_x_position_counter[sprite] -= 1;
                                }
                                
                                // Shift
                                self.sprite_pattern_shift_reg_hi[sprite] <<= 1;
                                self.sprite_pattern_shift_reg_lo[sprite] <<= 1;
                            }
                        }

                        match (self.tick - 1) % 8 {
                            0 => {
                                // Load the background shift registers with pattern table data
                                self.bg_pattern_shift_reg_hi = (self.bg_pattern_shift_reg_hi & 0xFF00) | self.bg_pattern_next_hi as u16;
                                self.bg_pattern_shift_reg_lo = (self.bg_pattern_shift_reg_lo & 0xFF00) | self.bg_pattern_next_lo as u16;

                                // Load the attribute shift registers with an expanded (8x1 slither) attribute value
                                self.bg_attribute_shift_reg_hi = (self.bg_attribute_shift_reg_hi & 0xFF00) | if self.bg_attribute_next_hi & 1 == 1 { 0xFF } else { 0x00 };
                                self.bg_attribute_shift_reg_lo = (self.bg_attribute_shift_reg_lo & 0xFF00) | if self.bg_attribute_next_lo & 1 == 1 { 0xFF } else { 0x00 };

                                self.bg_next_tile = self.read(NESPpu::tile_attr_from_vram_addr(self.vram_v).0);
                            }
                            2 => {
                                self.bg_next_attr = self.read(NESPpu::tile_attr_from_vram_addr(self.vram_v).1);
                            }
                            4 => {
                                // Get the lsb bit plane from the pattern table for the next tile
                                self.bg_pattern_next_lo = self.read(
                                    (self.ppu_ctrl.contains(PPUCTRL::BACKGROUND_TABLE_ADDR) as u16) << 12
                                |   (self.bg_next_tile as u16) << 4
                                |   ((self.vram_v & 0x7000) >> 12)); 
                            }
                            6 => {
                                // Get the msb bit plane from the pattern table for the next tile (+8 offset from LSB)
                                self.bg_pattern_next_hi = self.read(
                                    (self.ppu_ctrl.contains(PPUCTRL::BACKGROUND_TABLE_ADDR) as u16) << 12
                                |   (self.bg_next_tile as u16) << 4
                                |   ((self.vram_v & 0x7000) >> 12) + 8);
                            }
                            7 => {
                                // This is only done when rendering is enabled
                                if self.ppu_mask.contains(PPUMASK::RENDERING) {
                                    // Scroll horizontally (algorithm taken from NESDEV)
                                    if self.vram_v & 0x001F == 31 { // Are we at the end of a nametable?
                                        self.vram_v &= !0x001F;     // Reset course X to 0
                                        self.vram_v ^= 0x0400;      // Switch the horizontal nametable
                                    } else {
                                        self.vram_v += 1; // Increment as usual :-)
                                    }
                                }
                            }
                            _ => {}
                        }
                    }

                    if self.tick == 256 {
                        // When we reach the end of a scanline, increment the fine Y-scroll, then course vertical scroll.
                        // Again, this algorithm is lovingly taken from NESDEV.
                        // This is only done when rendering is enabled
                        if self.ppu_mask.contains(PPUMASK::RENDERING) {
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

                    // Superfluous nametable reads at end of scanline
                    if self.tick == 338 || self.tick == 340 {
                        self.bg_next_tile = self.read(0x2000 | (self.vram_v & 0x0FFF));
                    }

                    // Sprite evaluation and fetching. TODO: Intermingle this with background rendering
                    if self.ppu_mask.contains(PPUMASK::RENDERING) {
                        match self.tick {
                            1..=32 => {
                                // Initialize sOAM with $FF
                                self.soam[self.tick as usize - 1] = 0xFF;
                            }
                            65..=256 => {
                                if self.tick % 2 == 1 { // Odd cycles - read
                                    match self.sprite_evaluation_substage {
                                        1 => {
                                            self.poam_data = self.poam[4*self.poam_sprite_index + self.poam_sprite_byte_index];
                                        }
                                        2 => {
                                            // Increment n is not handled on odd cycles
                                        }
                                        // Sprite overflow checks ...
                                        3 => {
                                            self.poam_data = self.poam[4*self.poam_sprite_index + self.poam_sprite_byte_index];
            
                                            if (self.poam_data..(self.poam_data+8)).contains(&(self.scanline as u8)) {
                                                self.ppu_status.insert(PPUSTATUS::SPRITE_OVERFLOW);
                                                self.poam_sprite_byte_index += 1;
            
                                                if self.poam_sprite_byte_index >= 4 {
                                                    self.poam_sprite_index += 1;
                                                    self.poam_sprite_byte_index = 0;
                                                }
                                            } else {
                                                self.poam_sprite_byte_index += 1;    // Hardware bug
                                                // Wrap sprite_byte_index, but don't carry into sprite_index
                                                if self.poam_sprite_byte_index >= 4 {
                                                    self.poam_sprite_byte_index = 0;
                                                }
                                                self.poam_sprite_index += 1;
                                                if self.poam_sprite_index >= 64 {
                                                    self.sprite_evaluation_substage = 4;
                                                } else {
                                                    self.poam_sprite_byte_index = 0; // Effectively restart stage 3
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                } else { // Even cycles - write
                                    match self.sprite_evaluation_substage {
                                        1 => {
                                            // Are the writing the three other bytes from a hit?
                                            if self.sprite_evaluation_write_rest {
                                                self.soam[4*self.soam_next_open_slot + self.poam_sprite_byte_index] = self.poam_data;
                                                self.poam_sprite_byte_index += 1;
                                                if self.poam_sprite_byte_index >= 4 {
                                                    // We're finished here
                                                    self.sprite_evaluation_write_rest = false;
                                                    self.poam_sprite_byte_index = 0;
                                                    self.soam_next_open_slot += 1;
                                                    self.sprite_evaluation_substage = 2;
                                                }
                                            } else {
                                                if self.soam_next_open_slot <= 7 { // We have sOAM slots available
                                                    // Use the same byte index as pOAM to determine where in slot to write.
                                                    self.soam[4*self.soam_next_open_slot + self.poam_sprite_byte_index] = self.poam_data;
                                                }
                                                // Check if that y-coordinate is of interest to us
                                                // In other words, is this scanline in the range [poam_data, poam_data + 8) ?
                                                if (self.poam_data..(self.poam_data+8)).contains(&(self.scanline as u8)) {
                                                    self.poam_sprite_byte_index += 1;
                                                    self.sprite_evaluation_write_rest = true;
                                                } else {
                                                    self.poam_sprite_byte_index = 0;
                                                    self.soam_next_open_slot += 1;
                                                    self.sprite_evaluation_substage = 2;
                                                }
                                            }
                                        }
                                        2 => {
                                            // Increment poam_sprite_index
                                            self.poam_sprite_index += 1;
                                            if self.poam_sprite_index >= 64 {
                                                // Overflow
                                                // This will go back to zero, meaning that sprite evaluation
                                                // will continue to try to fit sprite 0 (or first matching sprite)
                                                // into sOAM, but fail each time (because it is full)
                                                self.poam_sprite_index = 0; // Overflow
                                                self.sprite_evaluation_substage = 4;
                                            } else if self.soam_next_open_slot <= 7 {
                                                self.sprite_evaluation_substage = 1;
                                            } else if self.soam_next_open_slot == 8 {
                                                self.sprite_evaluation_substage = 3;
                                                // You're supposed to disable writes in this case.
                                                // However, the logic here is such that writes just won't
                                                // get done anyways.
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            257..=320 => { 
                                // First, use this opportunity to reset the state
                                // of the sprite evaluation phase
                                if self.tick == 257 {
                                    self.sprite_evaluation_substage = 1;
                                    self.soam_next_open_slot = 0;
                                    self.poam_sprite_index = 0;
                                    self.poam_sprite_byte_index = 0;
                                }

                                // Fetch tile data for sprites on the next scanline
                                // For each sprite... (8 sprites, 8 cycles per sprite = 64 cycles)
                                // ... read data from sOAM for the first four cycles.
                                //     (the PPU is doing garbage fetches to the nametables during this time TODO)
                                // ... fetch pattern table tile lo and hi in last four cycles.
                                //     (remembering classic read-write cadence)
                                let soam_sprite_index: usize = (((self.tick - 1) & 0x38) >> 3) as usize;
                                match (self.tick - 1) % 8 {
                                    0 => { // First tick of 1st garbage nametable byte
                                        // (Emulation cheat)
                                        // Just load all information about current sprite.
                                        // NESDEV is once again questionable here...
                                        // ... PPU_sprite_evaluation states that reads are done to "Y-coordinate, tile number, ..."
                                        // ... etc. during cycles 1-4. But that ordering must be incorrect as
                                        // ... PPU_rendering states that X-positions and attributes are latched starting
                                        // ... tick 2 (tick 0 of 2nd garbage nametable byte). That would mean those pieces of
                                        // ... data are written before having even been loaded... ?
                                        // ... My guess here is that the sprite data from sOAM is read backwards.
                                        // ... ... which seems wrong.

                                        // Try to avoid this ambiguity by just loading everything in one go here.
                                        self.sprite_render_oam_y_coord = self.soam[soam_sprite_index * 4 + 0];
                                        self.sprite_render_oam_tile_number = self.soam[soam_sprite_index * 4 + 1];
                                        self.sprite_render_oam_attribute = self.soam[soam_sprite_index * 4 + 2];
                                        self.sprite_render_oam_x_coord = self.soam[soam_sprite_index * 4 + 3];
                                    }
                                    2 => { // First tick of 2nd garbage nametable byte
                                        // See moan above. Write attribute byte to latch
                                        self.sprite_attribute_latch[soam_sprite_index] = self.sprite_render_oam_attribute;
                                    }
                                    3 => { // Second tick of 2nd garbage nametable byte
                                        self.sprite_x_position_counter[soam_sprite_index] = self.sprite_render_oam_x_coord;
                                    }
                                    4 => {
                                        // fine-y in sprites depends on the flip attribute of this sprite
                                        let fine_y = if self.sprite_render_oam_attribute & (1 << 7) > 0 {
                                            (self.scanline - self.sprite_render_oam_y_coord as u16) & 0x7}
                                            else {(7 - (self.scanline - self.sprite_render_oam_y_coord as u16)) & 0x7};

                                        self.sprite_pattern_shift_reg_hi[soam_sprite_index] = self.read(
                                            (self.ppu_ctrl.contains(PPUCTRL::SPRITE_TABLE_ADDR) as u16) << 12
                                        |   (self.sprite_render_oam_tile_number as u16) << 4
                                        |   fine_y);

                                        // If the sprite is flipped horizontally, we need to invert the order of bits
                                        if self.sprite_render_oam_attribute & (1 << 6) > 0 {
                                            let mut t = self.sprite_pattern_shift_reg_hi[soam_sprite_index];
                                            t = (t & 0xF0) >> 4 | (t & 0x0F) << 4;
                                            t = (t & 0xCC) >> 2 | (t & 0x33) << 2;
                                            t = (t & 0xAA) >> 1 | (t & 0x55) << 1;
                                            self.sprite_pattern_shift_reg_hi[soam_sprite_index] = t;
                                        }
                                    }
                                    6 => {
                                        // fine-y in sprites depends on the flip attribute of this sprite
                                        let fine_y = if self.sprite_render_oam_attribute & (1 << 7) > 0 {
                                            (self.scanline - self.sprite_render_oam_y_coord as u16) & 0x7}
                                            else {(7 - (self.scanline - self.sprite_render_oam_y_coord as u16)) & 0x7};

                                        self.sprite_pattern_shift_reg_lo[soam_sprite_index] = self.read(
                                            (self.ppu_ctrl.contains(PPUCTRL::SPRITE_TABLE_ADDR) as u16) << 12
                                        |   (self.sprite_render_oam_tile_number as u16) << 4
                                        |   (fine_y + 8));

                                        // If the sprite is flipped horizontally, we need to invert the order of bits
                                        if self.sprite_render_oam_attribute & (1 << 6) > 0 {
                                            let mut t = self.sprite_pattern_shift_reg_lo[soam_sprite_index];
                                            t = (t & 0xF0) >> 4 | (t & 0x0F) << 4;
                                            t = (t & 0xCC) >> 2 | (t & 0x33) << 2;
                                            t = (t & 0xAA) >> 1 | (t & 0x55) << 1;
                                            self.sprite_pattern_shift_reg_lo[soam_sprite_index] = t;
                                        }
                                    }
                                    _ => { }
                                }
                            }  
                            _ => { }
                        }
                    }
                }
                241..=260 => {
                    if self.scanline == 241 && self.tick == 1 {
                        self.ppu_status.insert(PPUSTATUS::VBLANK);
                        if self.ppu_ctrl.contains(PPUCTRL::NMI_ENABLED) {
                            self.cpu.borrow_mut().do_nmi = true;
                        }
                    }
                }
                _ => {}
            }

            // SPRITE PIXEL BEGIN
            // Generate a sprite pixel, including transformations from attribute byte
            let mut sp_pixel: u8 = 0;         /* An index into a palette */
            let mut sp_palette: u8 = 0;       /* Which palette are we indexing? */
            let mut sp_latch: u8 = 0;         /* Attibute latch of sprite for live pixel */
            let mut sp_is_zero: bool = false;

            if self.ppu_mask.contains(PPUMASK::SPRITES) {
                // Decrement all of the sprite x-position counters by 1
                // Sprites are checked in the order they were placed in OAM
                for sprite_counter in 0usize..8usize {
                    if self.sprite_x_position_counter[sprite_counter] == 0 { 
                        // Palette handled quite differently from background
                        sp_palette = (self.sprite_attribute_latch[sprite_counter] & 0x3) + 4;
                        
                        // Also need the accepted sprite's attribute latch so we can determine priority
                        sp_latch = self.sprite_attribute_latch[sprite_counter];

                        // Retrieve the pattern - easier than bg evaluation because
                        // we've already done all the heavy lifting
                        let lbp_pattern = ((self.sprite_pattern_shift_reg_lo[sprite_counter] & 0x80) > 0) as u8;
                        let hbp_pattern = ((self.sprite_pattern_shift_reg_hi[sprite_counter] & 0x80) > 0) as u8;
                        
                        sp_pixel = (hbp_pattern << 1) | lbp_pattern;

                        // If the sprite pixel here is non-zero, break early
                        if sp_pixel > 0 {
                            // If this is sprite zero, need to cater for sprite zero hit later on
                            if sprite_counter == 0 {
                                sp_is_zero = true;
                            }
                            break;
                        }
                    }
                }
            }
            
            // SPRITE PIXEL END
            // BACKGROUND PIXEL BEGIN

            let mut bg_pixel: u8 = 0;    /* An index into a palette */
            let mut bg_palette: u8 = 0;  /* Which palette are we indexing? */

            if self.ppu_mask.contains(PPUMASK::BACKGROUND) {
                // Retrieve the pattern information, indexing with fine_x
                let lbp_pattern = ((self.bg_pattern_shift_reg_lo & (0x8000 >> self.vram_x)) > 0) as u8;
                let hbp_pattern = ((self.bg_pattern_shift_reg_hi & (0x8000 >> self.vram_x)) > 0) as u8;
                
                bg_pixel = (hbp_pattern << 1) | lbp_pattern;

                // Now let's get the corresponding palette information
                let lbp_attribute = ((self.bg_attribute_shift_reg_lo & (0x8000 >> self.vram_x)) > 0) as u8;
                let hbp_attribute = ((self.bg_attribute_shift_reg_hi & (0x8000 >> self.vram_x)) > 0) as u8;

                bg_palette = (hbp_attribute << 1) | lbp_attribute;
            }

            // BACKGROUND PIXEL END
            // MUX BEGIN
            let mut out_pixel: u8 = 0;  /* The value of the pixel ultimately output to the screen */
            let mut out_palette: u8 = 0;  /* The palette for this pixel */

            // We need to handle both pixel ordering
            // and sprite zero hits (when rendering falls through to a transparent pixel (sprite 0))
            
            // Forceground priority and BG is zero - sprite wins!
            if sp_pixel != 0 && (sp_latch & (1 << 5) == 0 || bg_pixel == 0) {
                out_pixel = sp_pixel;
                out_palette = sp_palette;
            } else {
                out_pixel = bg_pixel;
                out_palette = bg_palette;
            }

            // Handle sprite zero hit
            // This occurs when an opaque pixel from sprite 0 overlaps an
            // opaque pixel of the background


            // MUX END
            // OUTPUT BEGIN

            // Read palette RAM to determine which colour code this pixel is
            let out_pixel_colour = self.read(0x3F00 | ((out_palette as u16) << 2) | (out_pixel as u16));

            // Add this colour code to the pixel array, only if we are in the visible region.
            // Note that on a real NES, the first pixel output is not produced until tick = 4
            if self.scanline >= 0 && self.scanline <= 239
                && self.tick >= 1 && self.tick <= 256 {
                    self.frame[self.scanline as usize * 256 + (self.tick as usize - 1)] = out_pixel_colour;
            }

            self.tick += 1;
            if self.tick >= 341 {
                self.tick = 0;
                self.scanline += 1;
                if self.scanline >= 262 {
                    self.scanline = 0;
                    self.frame_ready = true;
                }
            }

            // OUTPUT END
        }
    }
}