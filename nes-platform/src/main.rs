use std::collections::binary_heap::Iter;
use std::fs;
use std::env;
use std::path::PathBuf;
use std::thread::current;
use clap::{ArgEnum, Parser};
use nes::cpu::NESCpu;
use nes::cpu::debug::disasm_6502;
use nes::cpu::decode::LUT_6502;
use nes_platform::load_palette;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::rect::Rect;
use sdl2::render::TextureQuery;

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

    /// Path to a .pal (palette) file
    #[clap(short, required = true, parse(from_os_str))]
    palette: PathBuf,

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

    let mut prg_rom_data = vec![0; nes_rom_header.prg_rom_size as usize];
    if nes_rom_header.has_trainer {
        println!("ROM has trainer - ignoring.");
        prg_rom_data.copy_from_slice(&nes_rom[528..(528 + nes_rom_header.prg_rom_size as usize)]);
    } else {
        prg_rom_data.copy_from_slice(&nes_rom[16..(16+nes_rom_header.prg_rom_size as usize)]);
    }

    cpu.memory.cartridge_mapper.load_prg_rom(&prg_rom_data);

    let palette = load_palette(args.palette);

    cpu.reset();
    
    let mut disasm_strings: Vec<String> = vec![];
    let mut disasm_string: String;
    let mut current_addr: u16 = cpu.PC;
    let mut last_offset: u16;

    (disasm_string, last_offset) = disasm_6502(current_addr, &mut cpu.memory);
    disasm_strings.push(format!("${:X}: {}", current_addr, disasm_string));
    current_addr += last_offset;

    for _ in 0..25 {
        (disasm_string, last_offset) = disasm_6502(current_addr, &mut cpu.memory);
        disasm_strings.push(format!("${:X}: {}", current_addr, disasm_string));
        current_addr += last_offset;
    }

    let mut disasm_sel: usize = 0;

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();

    let window = video_subsystem.window("fancy-nes v0.1.0", 256 * 2 + 180, 240 * 2)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let font = ttf_context.load_font("debug.ttf", 13).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), ..} => {
                    break 'running
                },
                Event::KeyDown { keycode: Some(Keycode::Down), ..} => {
                    if disasm_sel < 25 {
                        disasm_sel += 1;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Up), ..} => {
                    if disasm_sel > 0 {
                        disasm_sel -= 1;
                    }
                }
                _ => {}
            }
        }

        let surface = font
            .render(
            disasm_strings.iter().enumerate()
                .map(|i| {
                    if i.0 == disasm_sel {
                        "> ".to_owned() + i.1
                    } else {
                        "  ".to_owned() + i.1
                    }
                }).collect::<Vec<String>>().join("\n").as_str()
            )
            .blended_wrapped(Color::RGBA(255, 255, 255, 255), 160)
            .map_err(|e| e.to_string()).unwrap();

        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string()).unwrap();

        let TextureQuery { width, height, .. } = texture.query();

        let text_rect = Rect::new(256*2+10, 10, width, height);

        canvas.clear();
        canvas.set_draw_color(Color::RGBA(0, 0, 255, 180));
        canvas.fill_rect(Rect::new(256*2, 0, 180, 240*2)).unwrap();

        canvas.copy(&texture, None, Some(text_rect)).unwrap();

        canvas.present();
    }
}