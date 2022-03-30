//use core::fmt;

pub mod cpu;

#[derive(Debug)]
pub enum Mirroring {
    Horizontal,  /* vertical arrangement */
    Vertical,    /* horizontal arrangement */
    FourScreen, 
}

#[derive(Debug)]
pub struct NESHeaderMetadata {
    pub hardwired_mirroring: Mirroring,
    pub mapper_id: u8,
    pub prg_rom_size: u32,
    pub chr_rom_size: u32,
    pub has_trainer: bool,
}

struct NESHeader {
    prg_rom: u8,
    chr_rom: u8,
    flags6: u8,
    flags7: u8,
    mapper: u8,          /* NES2.0 */
    prg_chr_msb: u8,     /* NES2.0 */
    prg_eeprom_sz: u8,   /* NES2.0 */
    cpu_ppu_timing: u8,  /* NES2.0 */
    hw_type: u8,         /* NES2.0 */
    misc_roms: u8,       /* NES2.0 */
    exp_device: u8,      /* NES2.0 */
}

impl NESHeaderMetadata {
    pub fn parse_header(header: &Vec<u8>) -> Result<Self, &'static str> {
       if header[0..=3] != [b'N', b'E', b'S', 0x1A] {
           return Err("Header missing NES<EOF> magic");
       }

       /* place fields in human-readable struct */
       let nes_header = NESHeader {
           prg_rom: header[4],
           chr_rom: header[5],
           flags6: header[6],
           flags7: header[7],
           mapper: header[8],
           prg_chr_msb: header[9],
           prg_eeprom_sz: header[10],
           cpu_ppu_timing: header[11],
           hw_type: header[12],
           misc_roms: header[13],
           exp_device: header[14]
       };

       /* check whether this is a "NES2.0" or "iNES"-style header */
       let is_nes2 = (nes_header.flags7 & 0b1100) == 0b1000;
       
       /* bit 3 takes priority and indicates FourScreen mirroring.
          otherwise use bits 0-1 to determine Horizontal or Vertical mirroring. 
          */
       let hardwired_mirroring = if nes_header.flags6 & (1 << 3) == 1 {
            Mirroring::FourScreen
       } else {
            match nes_header.flags6 & 1 {
                0 => Mirroring::Horizontal,
                _ => Mirroring::Vertical,
            }
       };
       
       /* get mapper number from flags6 and flags7 */
       let mapper_id = (nes_header.flags6 & 0b11110000) >> 4
                         | (nes_header.flags7 & 0b11110000);

       /* get the size of the PRG ROM - declared in 16 KB units */
       let prg_rom_size = nes_header.prg_rom as u32 * 16 * 1024;

       /* get the size of the CHR ROM - declared in 8 KB units
        * may be 0, in which case only CHR RAM is used.
        */
       let chr_rom_size = nes_header.chr_rom as u32 * 8 * 1024;

       let has_trainer = nes_header.flags6 & 0x4 > 0;

       Ok(Self {
           hardwired_mirroring,
           mapper_id,
           prg_rom_size,
           chr_rom_size,
           has_trainer
       })
    }
}