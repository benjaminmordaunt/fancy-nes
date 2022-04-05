use std::cell::RefCell;
use std::fs;
use std::ops::Index;
use std::path::PathBuf;
use std::rc::Rc;
use clap::{ArgEnum, Parser};
use nes::cpu::{NESCpu, debug};
use nes::cpu::debug::disasm_6502;
use nes::ppu::NESPPU;
use nes_platform::debug_view::DebugView;
use nes_platform::{load_palette, NES_SCREEN_WIDTH, NES_SCREEN_HEIGHT, NES_DEBUGGER_WIDTH, NES_PPU_INFO_HEIGHT};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{TextureQuery, Texture};
use sdl2::render::TextureAccess::*;

enum CPUMode {
    SingleStep,
    Continuous,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ArgEnum, Debug)]
enum Region {
    NTSC,
    PAL
}
#[derive(Default)]
struct Margin {
    top: u32,
    bottom: u32,
    left: u32,
    right: u32
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

    /// Start ROM with debugger halted
    #[clap(short)]
    halted_debug: bool,

    /// Force a specific region
    #[clap(short, arg_enum)]
    region: Option<Region>,
}

fn main() {
    let args = Args::parse();

    let mut show_ppu_info = false;
    let mut palette_selected = 0;
    let mut show_debugger = args.halted_debug;

    let mut cpu_mode = if args.halted_debug { CPUMode::SingleStep } else { CPUMode::Continuous };
    let mut should_step = false;

    let nes_rom = fs::read(args.rom).unwrap();
    let nes_rom_header = nes::NESHeaderMetadata::parse_header(&nes_rom).unwrap();
    
    // Load the PRG and CHR roms
    let cpu_cell = Rc::new(RefCell::new(NESCpu::new(nes_rom_header.mapper_id as usize)));
    let mut ppu = NESPPU::new(Rc::clone(&cpu_cell));

    let mut prg_rom_data = vec![0; nes_rom_header.prg_rom_size as usize];
    let chr_rom_data: Vec<u8>;

    if nes_rom_header.has_trainer {
        println!("ROM has trainer - ignoring.");

        let i = nes_rom_header.prg_rom_size as usize;
        prg_rom_data.copy_from_slice(&nes_rom[528..(528 + i)]);
        chr_rom_data = nes_rom[(528 + i)..(528 + i + nes_rom_header.chr_rom_size as usize)].to_vec();
    } else {
        let i = nes_rom_header.prg_rom_size as usize;
        prg_rom_data.copy_from_slice(&nes_rom[16..(16+nes_rom_header.prg_rom_size as usize)]);
        chr_rom_data = nes_rom[(16 + i)..(16 + i + nes_rom_header.chr_rom_size as usize)].to_vec();
    }

    cpu_cell.borrow_mut().memory.cartridge_mapper.load_prg_rom(&prg_rom_data);
    ppu.load_chr_rom(&chr_rom_data);

    let palette = load_palette(args.palette);
    cpu_cell.borrow_mut().reset();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem.window("fancy-nes v0.1.0", 
        NES_SCREEN_WIDTH + (if args.halted_debug { NES_DEBUGGER_WIDTH } else { 0 } ), 
        NES_SCREEN_HEIGHT)
        .position_centered()
        .build()
        .unwrap();

    let pixel_format = window.window_pixel_format();

    let canvas_cell = Rc::new(RefCell::new(window.into_canvas().build().unwrap()));

    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
    let mut debug_view = DebugView::new(canvas_cell.borrow().texture_creator(), &ttf_context, Rc::clone(&cpu_cell), cpu_cell.borrow().PC);

    // Create the texture and buffer which we will write RGB data into
    let nes_texture_creator = canvas_cell.clone().borrow().texture_creator();
    let mut nes_texture: Texture = nes_texture_creator
        .create_texture(None, Streaming, 246, 240)
        .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    'running: loop {
        for event in event_pump.poll_iter() {
          match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), ..} => {
                    break 'running
                },
                // Event::KeyDown { keycode: Some(Keycode::Down), ..} => {
                //     if disasm_sel < 25 {
                //         disasm_sel += 1;
                //     }
                // }
                // Event::KeyDown { keycode: Some(Keycode::Up), ..} => {
                //     if disasm_sel > 0 {
                //         disasm_sel -= 1;
                //     }
                // }
                Event::KeyDown { keycode: Some(Keycode::Hash), ..} => {
                    show_ppu_info = !show_ppu_info;

                    canvas_cell.borrow_mut().window_mut().set_size(NES_SCREEN_WIDTH
                        + if show_debugger { NES_DEBUGGER_WIDTH } else { 0 }, 
                        NES_SCREEN_HEIGHT
                        + if show_ppu_info { NES_PPU_INFO_HEIGHT } else { 0 }).unwrap();
                }
                Event::KeyDown { keycode: Some(Keycode::Quote), ..} => {
                    show_debugger = !show_debugger;

                    canvas_cell.borrow_mut().window_mut().set_size(NES_SCREEN_WIDTH
                        + if show_debugger { NES_DEBUGGER_WIDTH } else { 0 }, 
                        NES_SCREEN_HEIGHT
                        + if show_ppu_info { NES_PPU_INFO_HEIGHT } else { 0 }).unwrap();
                }
                Event::KeyDown { keycode: Some(Keycode::Right), keymod: sdl2::keyboard::Mod::LALTMOD, ..} => {
                    if palette_selected < 7 {
                        palette_selected +=  1;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::Left), keymod: sdl2::keyboard::Mod::LALTMOD, ..} => {
                    if palette_selected > 0 {
                        palette_selected -=  1;
                    }
                }
                Event::KeyDown { keycode: Some(Keycode::N), ..} => {
                    should_step = true;
                }
                _ => {}
            }
        }

        {
            let mut canvas = canvas_cell.borrow_mut();
            canvas.set_draw_color(Color::RGBA(0, 0, 0, 255));
            canvas.clear();
        }

        match &cpu_mode {
            CPUMode::SingleStep => { if should_step { cpu_cell.borrow_mut().tick(); should_step = false; } }
            CPUMode::Continuous => { cpu_cell.borrow_mut().tick(); }
        }

        if show_debugger {
            {
                let pos = debug_view.addresses.iter().position(|x| *x == cpu_cell.borrow().PC);
                if pos.is_none() {
                    panic!("Queried address not present in disasssembler. Did scrolling fail?");
                } else {
                    debug_view.selected_entry = pos.unwrap();
                }

                let canvas = canvas_cell.borrow_mut();
                debug_view.render(canvas);
            }
        }

        // Illustrate the contents of the four background, and four sprite palettes
        let palette_view_margin = Margin { top: 3, left: 3, ..Margin::default() };
        let palette_margin = Margin { left: 5, ..Margin::default() };

        if show_ppu_info {
            {
                let mut canvas = canvas_cell.borrow_mut();

                canvas.set_draw_color(Color::RGBA(255, 255, 255, 255));
                canvas.draw_rects(&(0..8).into_iter().map(|v| {
                    Rect::new(palette_view_margin.left as i32 + 48 * v + palette_margin.left as i32 * v,
                        (NES_SCREEN_HEIGHT + palette_view_margin.top) as i32, 50, 14)
                }).collect::<Vec<Rect>>()).unwrap();

                // Show the currently selected palette.
                canvas.draw_rect(Rect::new(palette_view_margin.left as i32 - 1
                    + palette_selected * 48 + palette_selected * palette_margin.left as i32,
                (NES_SCREEN_HEIGHT + palette_view_margin.top) as i32 - 1, 52, 16)).unwrap();

                // Actually populate the palette information
                ppu.palette.chunks(4).enumerate().for_each(|i| {
                    let palette_idx = i.0;
                    let mut color_idx = 0;

                    for color in i.1 {
                        let color_rgb = palette[*color as usize];

                        canvas.set_draw_color(color_rgb);
                        canvas.fill_rect(Rect::new(palette_view_margin.left as i32 + 1
                            + palette_idx as i32 * 48 + palette_idx as i32 * palette_margin.left as i32
                            + color_idx * 12,
                        (NES_SCREEN_HEIGHT + palette_view_margin.top) as i32 + 1, 12, 12)).unwrap();

                        color_idx += 1;
                    }
                });
            }
        }

        {
            let mut canvas = canvas_cell.borrow_mut();
            
            // Render the complete image (this will not work for ROMs which change mid-frame)
            let mut tex_raw: [u8; 61440*4] = [0; 61440*4];
            
            ppu.render(|pixels| {
                pixels.iter().enumerate().for_each(
                    |x| {
                        tex_raw[x.0*4+0] = 0xFF;
                        (tex_raw[x.0*4+1], tex_raw[x.0*4+2], tex_raw[x.0*4+3]) = palette[*x.1 as usize].rgb();
                    }
                )
            });

            
            nes_texture.update(Rect::new(0, 0, 256, 240), &tex_raw, 4 * 256).unwrap();
            canvas.copy(&nes_texture, None, Some(Rect::new(0, 0, NES_SCREEN_WIDTH, NES_SCREEN_HEIGHT))).unwrap();

            canvas.present();
        }
    }
}