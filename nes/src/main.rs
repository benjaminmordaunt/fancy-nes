use std::fs;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    let nes_rom = fs::read(&args[1]).unwrap();
    let nes_rom_header = nes::NESHeaderMetadata::parse_header(nes_rom).unwrap();
    println!("nes_rom_header: {:?}", nes_rom_header);
}