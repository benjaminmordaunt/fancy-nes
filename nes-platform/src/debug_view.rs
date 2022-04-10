use std::ascii::AsciiExt;
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use nes::cpu::{NESCpu, StatusRegister};
use nes::cpu::debug::disasm_6502;
use nes::ppu::NESPPU;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator, TextureQuery};
use sdl2::surface;
use sdl2::ttf::Sdl2TtfContext;
use sdl2::pixels::Color;
use sdl2::video::{Window, WindowContext};

use crate::{NES_SCREEN_WIDTH, NES_DEBUGGER_WIDTH, NES_SCREEN_HEIGHT};

pub struct DebugView<'a> {
    /* The address list here may seem redundant, as addresses are stored in disasm,,
       however, this provides a quick lookup to the renderer when trying to pin the PC to a line */
    pub addresses: [u16; 21],             /* a list of the 20 addresses disassembled and visible */

    disasm: HashMap<u16, (String, u16)>, /* a map of memory addresses to a disasm entry */
    cpu: Rc<RefCell<NESCpu>>,            /* we need to keep the whole CPU Rc alive, instead of trying to immutably
                                        reference just cpu.memory */
    ppu: Rc<RefCell<NESPPU>>,

    font: sdl2::ttf::Font<'a, 'static>,
    texture_creator: TextureCreator<WindowContext>,
}


impl<'a> DebugView<'a> {
    // Create a DebugView which renders onto the given canvas populates the disasm
    // HashMap with some useful initial entries
    pub fn new(texture_creator: TextureCreator<WindowContext>, ttf_context: &'a Sdl2TtfContext, cpu: Rc<RefCell<NESCpu>>, ppu: Rc<RefCell<NESPPU>>) -> Self {        
        let mut result = Self {
            addresses: [0; 21],
            disasm: HashMap::new(),
            cpu: Rc::clone(&cpu),
            ppu: Rc::clone(&ppu),
            font: ttf_context.load_font("debug.ttf", 16).unwrap(),
            texture_creator
        };

        // Insert a null disassembly
        result.disasm.insert(0, ("-".to_string(), 0));

        DebugView::update_addresses(&mut result);

        result
    }

    fn update_addresses(&mut self) {
        // Update addresses in live address range

        // Clear addresses
        self.addresses = [0; 21];

        let mut disasm_string: String;
        let mut current_addr: u16 = self.cpu.borrow().PC;
        let mut on_screen_index: usize = 10;
        let mut initial_offset: u16;
        let mut last_offset: u16;

        // Forward pass
        for i in 0..11 {
            (disasm_string, last_offset) = self.disasm.get(&current_addr).unwrap_or(&disasm_6502(current_addr, &self.cpu.borrow().memory)).to_owned();
            if i == 0 {
                initial_offset = last_offset;
            }
            self.disasm.insert(current_addr, (disasm_string, last_offset));
            self.addresses[on_screen_index] = current_addr;
            on_screen_index += 1;
            current_addr += last_offset;
        }

        // Figure out how to do a backwards pass
    }

    pub fn render(&mut self, mut canvas: RefMut<Canvas<Window>>) {
        self.update_addresses();

        // Take a copy of the address disassemblies of interest and format appropriately.
        let disasm_vec = self.addresses.iter().enumerate()
            .map(|i| { if i.0 == 10 {
                format!("> ${:0>4X}: {}", i.1, self.disasm[i.1].0)
             } else {
                format!("  ${:0>4X}: {}", i.1, self.disasm[i.1].0)
             } 
        });
        
        // TODO - Integrate a better font rendering library so we are not constantly creating textures...

        let surface = self.font
            .render(
                disasm_vec.collect::<Vec<String>>().join("\n").as_str()
            )
            .blended_wrapped(Color::RGBA(255, 255, 255, 255), NES_DEBUGGER_WIDTH)
            .map_err(|e| e.to_string()).unwrap();

        let texture = self.texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string()).unwrap();


        let TextureQuery { width, height, .. } = texture.query();
        let text_rect = Rect::new(NES_SCREEN_WIDTH as i32 + 10, 10, width, height);
        
        canvas.set_draw_color(Color::RGBA(0, 0, 255, 180));
        canvas.fill_rect(Rect::new(NES_SCREEN_WIDTH as i32, 0, NES_DEBUGGER_WIDTH, NES_SCREEN_HEIGHT)).unwrap();

        canvas.copy(&texture, None, Some(text_rect)).unwrap();

        let cpu = self.cpu.borrow();
        let ppu = self.ppu.borrow();
        let mut status_string = String::new();
        status_string.push(if cpu.status.contains(StatusRegister::NEGATIVE) { 'N' } else { 'n' });
        status_string.push(if cpu.status.contains(StatusRegister::OVERFLOW) { 'V' } else { 'v' });
        status_string.push(if cpu.status.contains(StatusRegister::BREAK_HIGH) { 'B' } else { 'b' });
        status_string.push(if cpu.status.contains(StatusRegister::BREAK_LOW) { 'B' } else { 'b' });
        status_string.push(if cpu.status.contains(StatusRegister::DECIMAL_MODE) { 'D' } else { 'd' });
        status_string.push(if cpu.status.contains(StatusRegister::INTERRUPT_DISABLE) { 'I' } else { 'i' });
        status_string.push(if cpu.status.contains(StatusRegister::ZERO) { 'Z' } else { 'z' });
        status_string.push(if cpu.status.contains(StatusRegister::CARRY) { 'C' } else { 'c' });

        status_string.push_str(format!("\n\nA: {:0>2X} X: {:0>2X} Y: {:0>2X} SP: {:0>2X} scan: {} tick: {}", 
            cpu.A,
            cpu.X,
            cpu.Y,
            cpu.SP,
            ppu.scanline,
            ppu.tick,
        ).as_str());

        let surface = self.font
            .render(
                status_string.as_str()
            )
            .blended_wrapped(Color::RGBA(255, 255, 255, 255), NES_DEBUGGER_WIDTH)
            .map_err(|e| e.to_string()).unwrap();
        
        let texture = self.texture_creator
            .create_texture_from_surface(&surface)
            .map_err(|e| e.to_string()).unwrap();

        let TextureQuery { width, height, .. } = texture.query();
        let text_rect = Rect::new(NES_SCREEN_WIDTH as i32 + 10, 360, width, height);

        canvas.copy(&texture, None, Some(text_rect)).unwrap();
    }
}