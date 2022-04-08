/// Mappers need to describe how to handle addresses in the range 0x4020-0xFFFF.
/// In reality, most mappers don't handle addresses < $6000, where work RAM typically begins.

pub trait Mapper<Tr, Tw> {
    // Use the &self version for no-side-effect (fake) accesses, such as
    // querying memory for the disassembler
    fn read(&self, addr: u16) -> Tr;
    fn write(&mut self, addr: u16, data: u8) -> Tw;

    fn load_rom(&mut self, rom: &Vec<u8>);
}
