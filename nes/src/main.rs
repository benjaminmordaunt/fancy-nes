use std::fs;
use std::env;
use std::path::PathBuf;
use clap::{ArgEnum, Parser};
use cpu::NESCpu;
use cpu::debug::disasm_6502;
use cpu::decode::LUT_6502;

mod cpu;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum, Debug)]
enum Region {
    NTSC,
    PAL
}

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
/// fancy-nes Nintendo Entertainment System/Famicom Emulator
struct Args {
    /// Path to NES ROM image
    #[clap(required = true, parse(from_os_str))]
    rom: PathBuf,

    /// Force a specific region
    #[clap(short, arg_enum)]
    region: Option<Region>,
}

fn main() {
    let args = Args::parse();

    let nes_rom = fs::read(args.rom).unwrap();
    let nes_rom_header = nes::NESHeaderMetadata::parse_header(&nes_rom).unwrap();
    
    // Load the PRG and CHR roms
    let mut cpu = NESCpu::new(nes_rom_header.mapper_id as usize);

    let mut prg_rom_data: Vec<u8> = Vec::with_capacity(nes_rom_header.prg_rom_size as usize);
    if nes_rom_header.has_trainer {
        println!("ROM has trainer - ignoring.");
        prg_rom_data.copy_from_slice(&nes_rom[528..(528 + nes_rom_header.prg_rom_size as usize)]);
    } else {
        prg_rom_data.copy_from_slice(&nes_rom[16..(16+nes_rom_header.prg_rom_size as usize)]);
    }

    cpu.memory.cartridge_mapper.load_prg_rom(&prg_rom_data);
    cpu.reset();

    // Show the current value of the Program Counter
    println!("ROM's reset vector (in PC) after 6502 reset: ${:X}", cpu.PC);
    
    nes_platform::render_main();
}