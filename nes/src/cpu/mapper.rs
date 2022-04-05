/// Mappers need to describe how to handle addresses in the range 0x4020-0xFFFF.
/// In reality, most mappers don't handle addresses < $6000, where work RAM typically begins.

pub trait Mapper {
    // Use the &self version for no-side-effect (fake) accesses, such as
    // querying memory for the disassembler
    fn read(&self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);

    // Mapper activity within the PPU addressing range
    fn read_ppu(&self, addr: u16) -> u16;
    fn write_ppu(&mut self, addr: u16, data: u8) -> u16; // May need to change state inside PPU, so return info (see Mapperxxx)

    fn load_prg_rom(&mut self, rom: &Vec<u8>);
    fn load_chr_rom(&mut self, rom: &Vec<u8>);
}