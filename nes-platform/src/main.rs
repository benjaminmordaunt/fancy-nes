use std::cell::{RefCell, Ref};
use std::fs;
use std::ops::Index;
use std::path::PathBuf;
use std::rc::Rc;
use clap::{ArgEnum, Parser};
use nes::cpu::mapper000::CPUMapper000;
use nes::cpu::{NESCpu, debug};
use nes::cpu::debug::{disasm_6502, cpu_dump};
use nes::ppu::NESPPU;
use nes_platform::debug_view::DebugView;
use nes_platform::{load_palette, NES_SCREEN_WIDTH, NES_SCREEN_HEIGHT, NES_DEBUGGER_WIDTH, NES_PPU_INFO_HEIGHT};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{TextureQuery, Texture};
use sdl2::render::TextureAccess::*;
use sdl2::timer;

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

/* Flush the CPU's wait cycles. Invokes the appropriate number of PPU cycles */
fn flush_cpu(cpu: Rc<RefCell<NESCpu>>, ppu: Rc<RefCell<NESPPU>>) {
    while cpu.borrow().wait_cycles > 0 {
        if let Err(e) = cpu.borrow_mut().tick() {
            panic!("{}\nError: {}", cpu_dump(cpu.borrow()), e);
        }
        ppu.borrow_mut().ppu_tick(3); 
    }
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
    let mut ppu = Rc::new(RefCell::new(NESPPU::new(nes_rom_header.mapper_id as usize, Rc::clone(&cpu_cell), nes_rom_header.hardwired_mirroring)));

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

    cpu_cell.borrow_mut().memory.mapper.load_rom(&prg_rom_data);
    ppu.borrow_mut().mapper.load_rom(&chr_rom_data);

    let palette = load_palette(args.palette);
    cpu_cell.borrow_mut().reset();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let timer_subsystem = sdl_context.timer().unwrap();

    let mut window = video_subsystem.window("fancy-nes v0.1.0", 
        NES_SCREEN_WIDTH + (if args.halted_debug { NES_DEBUGGER_WIDTH } else { 0 } ), 
        NES_SCREEN_HEIGHT)
        .opengl()
        .position_centered()
        .build()
        .unwrap();

    let pixel_format = window.window_pixel_format();

    let canvas_cell = Rc::new(RefCell::new(window.into_canvas()
        .accelerated()
        .present_vsync()
        .build().unwrap()));

    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
    let mut debug_view = DebugView::new(canvas_cell.borrow().texture_creator(), &ttf_context, Rc::clone(&cpu_cell), Rc::clone(&ppu));

    // Create the texture and buffer which we will write RGB data into
    let nes_texture_creator = canvas_cell.clone().borrow().texture_creator();
    let mut nes_texture: Texture = nes_texture_creator
        .create_texture(None, Streaming, 246, 240)
        .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    // Connect the PPU's registers to the CPU's address space
    cpu_cell.borrow_mut().memory.ppu_registers = Some(ppu.clone());

    // Illustrate the contents of the four background, and four sprite palettes
    let palette_view_margin = Margin { top: 3, left: 3, ..Margin::default() };
    let palette_margin = Margin { left: 5, ..Margin::default() };

    // A thread handles emulating the CPU and PPU
    // and removes the overhead of SDL from the mix.
    // This allows us to determine shortfalls in emulator
    // performance separately from those incurred by SDL2.

    // Last update time
    let mut last_time: u64 = timer_subsystem.performance_counter();

    'running: loop {
        match &cpu_mode {
            CPUMode::SingleStep => { 
                if should_step { 
                    // In single-step mode, we need to fast-forward the CPU and
                    // PPU to the next instruction in order to provide "step-over"-like
                    // functionality in the debugger view.

                    // Perform a single tick anyways
                    if let Err(e) = cpu_cell.borrow_mut().tick() {
                        panic!("{}\nError: {}", cpu_dump(cpu_cell.borrow()), e);
                    }
                    ppu.borrow_mut().ppu_tick(3);

                    // Flush the pipeline
                    flush_cpu(Rc::clone(&cpu_cell), Rc::clone(&ppu));
                    should_step = false; 
                } 
            }
            CPUMode::Continuous => { 
                {
                    let mut cpu = cpu_cell.borrow_mut();
                    if let Err(e) = cpu.tick() {
                        panic!("{}\nError: {}", cpu_dump(cpu), e);
                    }
                }

                ppu.borrow_mut().ppu_tick(3); 

                // Simple breakpoint mechanism (make this programmable)
                // if cpu_cell.borrow().PC & 0xFFF0 == 0x8170 {
                //     // Finish processing this instruction
                //     flush_cpu(Rc::clone(&cpu_cell), Rc::clone(&ppu));

                //     cpu_mode = CPUMode::SingleStep;
                //     should_step = false;

                //     show_debugger = true;
                //     canvas_cell.borrow_mut().window_mut().set_size(NES_SCREEN_WIDTH
                //         + if show_debugger { NES_DEBUGGER_WIDTH } else { 0 }, 
                //         NES_SCREEN_HEIGHT
                //         + if show_ppu_info { NES_PPU_INFO_HEIGHT } else { 0 }).unwrap();
                // }
            }
        }

        let fps = (timer_subsystem.performance_frequency()) / (timer_subsystem.performance_counter() - last_time);

        // Place a minimum render rate of 30 FPS for when in single-step execution mode.
        if ppu.borrow().frame_ready || fps < 30 {
            // Set window title to be the FPS
            canvas_cell.borrow_mut().window_mut().set_title(format!("fancy-nes v0.1.0 - FPS: {}", fps).as_str()).unwrap();

            last_time = timer_subsystem.performance_counter();

            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit {..} |
                    Event::KeyDown { keycode: Some(Keycode::Escape), ..} => {
                        break 'running
                    },
                    Event::KeyDown { keycode: Some(Keycode::Hash), ..} => {
                        show_ppu_info = !show_ppu_info;

                        canvas_cell.borrow_mut().window_mut().set_size(NES_SCREEN_WIDTH
                            + if show_debugger { NES_DEBUGGER_WIDTH } else { 0 }, 
                            NES_SCREEN_HEIGHT
                            + if show_ppu_info { NES_PPU_INFO_HEIGHT } else { 0 }).unwrap();
                    }
                    Event::KeyDown { keycode: Some(Keycode::Quote), keymod: sdl2::keyboard::Mod::NOMOD, ..} => {
                        show_debugger = !show_debugger;

                        canvas_cell.borrow_mut().window_mut().set_size(NES_SCREEN_WIDTH
                            + if show_debugger { NES_DEBUGGER_WIDTH } else { 0 }, 
                            NES_SCREEN_HEIGHT
                            + if show_ppu_info { NES_PPU_INFO_HEIGHT } else { 0 }).unwrap();
                    }
                    Event::KeyDown { keycode: Some(Keycode::Quote), keymod: sdl2::keyboard::Mod::LALTMOD, ..} => {
                        cpu_mode = match cpu_mode {
                            CPUMode::SingleStep => CPUMode::Continuous,
                            CPUMode::Continuous => CPUMode::SingleStep,
                        }
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

            if show_debugger {
                {
                    let canvas = canvas_cell.borrow_mut();
                    debug_view.render(canvas);
                }
            }
    
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
                    ppu.borrow().palette.chunks(4).enumerate().for_each(|i| {
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

            // Render the complete image
            let mut tex_raw: [u8; 61440*4] = [0; 61440*4];
            
            for y in 0..240 {
                for x in 0..256 {
                    tex_raw[x * 4 + y * 256 * 4] = 0xFF; // Opaque
                    (tex_raw[x * 4 + 1 + y * 256 * 4],
                        tex_raw[x * 4 + 2 + y * 256 * 4],
                        tex_raw[x * 4 + 3 + y * 256 * 4])
                        = palette[ppu.borrow().frame[x + y * 256] as usize].rgb();
                }
            }
            
            nes_texture.update(Rect::new(0, 0, 256, 240), &tex_raw, 4 * 256).unwrap();
            ppu.borrow_mut().frame_ready = false;

            canvas_cell.borrow_mut().copy(&nes_texture, None, Some(Rect::new(0, 0, NES_SCREEN_WIDTH, NES_SCREEN_HEIGHT))).unwrap();
            canvas_cell.borrow_mut().present();
        }
    }
}