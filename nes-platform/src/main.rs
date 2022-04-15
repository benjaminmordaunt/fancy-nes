use std::cell::{RefCell, Ref};
use std::fs;
use std::ops::Index;
use std::path::{PathBuf, Path};
use std::rc::Rc;
use clap::{ArgEnum, Parser};
use nes::cpu::trace::TraceUnit;
use nes::cpu::{NESCpu, debug};
use nes::cpu::debug::{disasm_6502, cpu_dump};
use nes::ppu::NESPPU;
use nes_platform::debug_view::DebugView;
use nes_platform::{load_palette, NES_SCREEN_WIDTH, NES_SCREEN_HEIGHT, NES_DEBUGGER_WIDTH, NES_PPU_INFO_HEIGHT, NES_PPU_INFO_WIDTH};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::{Rect, Point};
use sdl2::render::{TextureQuery, Texture};
use sdl2::render::TextureAccess::*;
use sdl2::timer;

// Ensure that we aren't trying to use 2 different trace styles
#[cfg(all(feature = "fceux-log", feature = "nestest-log"))]
compile_error!("feature \"fceux-log\" and features \"nestest-log\" cannot be enabled at the same time");

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

fn get_screen_size(show_debugger: bool, show_ppu_info: bool) -> (u32, u32) {
    let width = NES_SCREEN_WIDTH + if show_debugger { NES_DEBUGGER_WIDTH } else { 0 }
                                      + if show_ppu_info { NES_PPU_INFO_WIDTH } else { 0 }; 

    let height = NES_SCREEN_HEIGHT + if show_ppu_info { NES_PPU_INFO_HEIGHT } else { 0 };

    (width, height)
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

    // Controller status
    let mut joy1 = RefCell::new(0 as u8);
    
    // Load the PRG and CHR roms
    let cpu_cell = Rc::new(RefCell::new(NESCpu::new(nes_rom_header.mapper_id as usize, &joy1)));
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
    let mut trace_unit: Option<TraceUnit> = None;

    cpu_cell.borrow_mut().reset();
    #[cfg(all(debug_assertions, feature = "nestest-log"))] 
    {
        let mut cpu = cpu_cell.borrow_mut();

        // In debug mode, we are loading nestest.nes ROM without PPU support.
        // Also, the trace unit is attached to provide instruction + cycle logs.
        cpu.PC = 0xC000;

        // Start with some dummy address on the stack (0x0800)
        cpu.SP = 0xFF;
        cpu.A = 0x00;
        cpu.op_stack_push(false);
        cpu.A = 0x08;
        cpu.op_stack_push(false);
        cpu.A = 0x00;

        // nestest.log starts with 7 cycles
        cpu.cycle = 7;

        trace_unit = Some(TraceUnit::new(Path::new("out.log")));
    }
    #[cfg(all(debug_assertions, feature = "fceux-log"))]
    {
        // We can just start trace_unit without any hacks
        trace_unit = Some(TraceUnit::new(Path::new("out.log")));
    }

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
        .create_texture_streaming(PixelFormatEnum::RGB24, 256, 240)
        .unwrap();

    let mut palette_texture = nes_texture_creator
        .create_texture_streaming(PixelFormatEnum::RGB24, 128, 128)
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
                    if let Some(ref mut tu) = trace_unit {
                        if cpu_cell.borrow().wait_cycles == 0 {
                            tu.dump(&cpu_cell.borrow());
                        }
                    }
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
                    if let Some(ref mut tu) = trace_unit {
                        if cpu_cell.borrow().wait_cycles == 0 {
                            tu.dump(&cpu_cell.borrow());
                        }
                    }
                    let mut cpu = cpu_cell.borrow_mut();
                    if let Err(e) = cpu.tick() {
                        panic!("{}\nError: {}", cpu_dump(cpu), e);
                    }
                }

                ppu.borrow_mut().ppu_tick(3); 

                // Simple breakpoint mechanism (make this programmable)
                if cpu_cell.borrow().PC & 0xFFFF == 0xC293 {
                    // Finish processing this instruction
                    flush_cpu(Rc::clone(&cpu_cell), Rc::clone(&ppu));

                    cpu_mode = CPUMode::SingleStep;
                    should_step = false;

                    show_debugger = true;
                    let size = get_screen_size(show_debugger, show_ppu_info);
                    canvas_cell.borrow_mut().window_mut().set_size(size.0, size.1).unwrap();
                }
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

                        let size = get_screen_size(show_debugger, show_ppu_info);
                        canvas_cell.borrow_mut().window_mut().set_size(size.0, size.1).unwrap();
                    }
                    Event::KeyDown { keycode: Some(Keycode::Quote), keymod: sdl2::keyboard::Mod::NOMOD, ..} => {
                        show_debugger = !show_debugger;

                        let size = get_screen_size(show_debugger, show_ppu_info);
                        canvas_cell.borrow_mut().window_mut().set_size(size.0, size.1).unwrap();
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

                    // Controller Port 1 BEGIN
                    /* A */
                    Event::KeyDown { keycode: Some(Keycode::Z), ..} => {
                        *joy1.borrow_mut() |= 1 << 0;
                    }
                    Event::KeyUp { keycode: Some(Keycode::Z), ..} => {
                        *joy1.borrow_mut() &= !(1 << 0);
                    }

                    /* B */
                    Event::KeyDown { keycode: Some(Keycode::X), ..} => {
                        *joy1.borrow_mut() |= 1 << 1;
                    }
                    Event::KeyUp { keycode: Some(Keycode::X), ..} => {
                        *joy1.borrow_mut() &= !(1 << 1);
                    }

                    /* Select */
                    Event::KeyDown { keycode: Some(Keycode::RShift), ..} => {
                        *joy1.borrow_mut() |= 1 << 2;
                    }
                    Event::KeyUp { keycode: Some(Keycode::RShift), ..} => {
                        *joy1.borrow_mut() &= !(1 << 2);
                    }

                    /* Start */
                    Event::KeyDown { keycode: Some(Keycode::Return), ..} => {
                        *joy1.borrow_mut() |= 1 << 3;
                    }
                    Event::KeyUp { keycode: Some(Keycode::Return), ..} => {
                        *joy1.borrow_mut() &= !(1 << 3);
                    }

                    /* Up */
                    Event::KeyDown { keycode: Some(Keycode::Up), ..} => {
                        *joy1.borrow_mut() |= 1 << 4;
                    }
                    Event::KeyUp { keycode: Some(Keycode::Up), ..} => {
                        *joy1.borrow_mut() &= !(1 << 4);
                    }

                    /* Down */
                    Event::KeyDown { keycode: Some(Keycode::Down), ..} => {
                        *joy1.borrow_mut() |= 1 << 5;
                    }
                    Event::KeyUp { keycode: Some(Keycode::Down), ..} => {
                        *joy1.borrow_mut() &= !(1 << 5);
                    }

                    /* Left */
                    Event::KeyDown { keycode: Some(Keycode::Left), keymod: sdl2::keyboard::Mod::NOMOD, ..} => {
                        *joy1.borrow_mut() |= 1 << 6;
                    }
                    Event::KeyUp { keycode: Some(Keycode::Left), keymod: sdl2::keyboard::Mod::NOMOD, ..} => {
                        *joy1.borrow_mut() &= !(1 << 6);
                    }

                    /* Right */
                    Event::KeyDown { keycode: Some(Keycode::Right), ..} => {
                        *joy1.borrow_mut() |= 1 << 4;
                    }
                    Event::KeyUp { keycode: Some(Keycode::Right), ..} => {
                        *joy1.borrow_mut() &= !(1 << 4);
                    }
                    // Controller Port 1 END
                    _ => {}
                }
            }

            // Render the complete image
            nes_texture.with_lock(None, |r, p| {
                for y in 0..240 {
                    for x in 0..256 {
                        let offset = y * p + x * 3;
                        let color = palette[ppu.borrow().frame[(y * 256 + x) as usize] as usize];
                        r[offset + 0] = color.r;  // R
                        r[offset + 1] = color.g;  // G
                        r[offset + 2] = color.b;  // B
                    }
                }
            }).unwrap();

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

                    // Draw the two pattern tables
                    canvas.set_draw_color(Color::RGBA(255, 255, 255, 255));
                    canvas.draw_rects(&(0..2).into_iter().map(|v| {
                        Rect::new(palette_view_margin.left as i32 + 256 * v + palette_margin.left as i32 * v,
                            (NES_SCREEN_HEIGHT + palette_view_margin.top * 2 + 14) as i32, 258, 258)
                    }).collect::<Vec<Rect>>()).unwrap();

                    {
                        let p_ppu = ppu.borrow();

                        let mut palette_raw = [0 as u8; 3*128*128];

                        for table in 0..2 {
                            for tile_row in 0..16 {
                                for tile_col in 0..16 {
                                    for fine_y in 0..8 {
                                        let lsb_addr: u16 = ((table << 12) | (tile_row << 8) | (tile_col << 4) | fine_y) as u16;

                                        let px_color_lsb = p_ppu.read(lsb_addr);
                                        let px_color_msb = p_ppu.read(lsb_addr + 8);

                                        for pxidx in 0..8 {
                                            let px_color = (((px_color_msb & (0x80 >> pxidx) > 1) as u8) << 1) | ((px_color_lsb & (0x80 >> pxidx) > 1) as u8);
                                            let px_color_pal = p_ppu.read(0x3F00 + px_color as u16);
                                            let px_color_rgb = palette[px_color_pal as usize];

                                            let draw_x = pxidx as i32 + 8 * tile_col as i32;
                                            let draw_y = fine_y as i32 + 8 * tile_row as i32;

                                            palette_raw[(draw_x * 3 + draw_y * 3 * 128 + 0) as usize] = px_color_rgb.r;
                                            palette_raw[(draw_x * 3 + draw_y * 3 * 128 + 1) as usize] = px_color_rgb.g;
                                            palette_raw[(draw_x * 3 + draw_y * 3 * 128 + 2) as usize] = px_color_rgb.b;
                                        }
                                    }
                                }
                            }
                            palette_texture.with_lock(None, |buffer: &mut [u8], pitch: usize| {
                                for y in 0..128 {
                                    for x in 0..128 {
                                        let offset = y * pitch + x * 3;
                                        buffer[offset] = palette_raw[x * 3 + y * 3 * 128 + 0];
                                        buffer[offset + 1] = palette_raw[x * 3 + y * 3 * 128 + 1];
                                        buffer[offset + 2] = palette_raw[x * 3 + y * 3 * 128 + 2];
                                    }
                                }
                            }).unwrap();
                            canvas.copy(&palette_texture, None, Some(Rect::new(
                                palette_view_margin.left as i32 + 256i32 * (table as i32) + palette_margin.left as i32 * (table as i32) + 1,
                                (NES_SCREEN_HEIGHT + palette_view_margin.top) as i32 + palette_margin.top as i32 + 18i32,
                                256, 256))).unwrap();
                        }   
                    }
                }
            }

            ppu.borrow_mut().frame_ready = false;

            canvas_cell.borrow_mut().copy(&nes_texture, None, Some(Rect::new(0, 0, NES_SCREEN_WIDTH, NES_SCREEN_HEIGHT))).unwrap();
            canvas_cell.borrow_mut().present();

            // Abort if > 1 million cycles have been traced.
            #[cfg(all(debug_assertions, feature = "fceux-log"))]
            {
                if cpu_cell.borrow().cycle > 1_000_000 {
                    break 'running;
                }
            }
        }
    }
}