/// Mappers need to describe how to handle addresses in the range 0x4020-0xFFFF.
/// In reality, most mappers don't handle addresses < $6000, where work RAM typically begins.

pub trait Mapper {
    fn read(&mut self, addr: u16) -> u8;
    fn write(&mut self, addr: u16, data: u8);
}