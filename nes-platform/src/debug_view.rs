use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::rc::Rc;
use nes::cpu::NESCpu;
use nes::cpu::debug::disasm_6502;
use sdl2::rect::Rect;
use sdl2::render::{Canvas, TextureCreator, TextureQuery};
use sdl2::ttf::Sdl2TtfContext;
use sdl2::pixels::Color;
use sdl2::video::{Window, WindowContext};

use crate::{NES_SCREEN_WIDTH, NES_DEBUGGER_WIDTH, NES_SCREEN_HEIGHT};

pub struct DebugView<'a> {
    pub selected_entry: usize,        /* the currenty selected position, where the cursor should be rendered */

    /* The address list here may seem redundant, as addresses are stored in disasm,,
       however, this provides a quick lookup to the renderer when trying to pin the PC to a line */
    pub addresses: [u16; 20],             /* a list of the 20 addresses disassembled and visible */

    disasm: HashMap<u16, String>, /* a map of memory addresses to a disasm entry */
    cpu: Rc<RefCell<NESCpu>>,            /* we need to keep the whole CPU Rc alive, instead of trying to immutably
                                        reference just cpu.memory */

    font: sdl2::ttf::Font<'a, 'static>,
    texture_creator: TextureCreator<WindowContext>,
}


impl<'a> DebugView<'a> {
    // Create a DebugView which renders onto the given canvas populates the disasm
    // HashMap with some useful initial entries
    pub fn new(texture_creator: TextureCreator<WindowContext>, ttf_context: &'a Sdl2TtfContext, cpu: Rc<RefCell<NESCpu>>, begin_address: u16) -> Self {        
        let mut result = Self {
            selected_entry: 0,
            addresses: [0; 20],
            disasm: HashMap::new(),
            cpu: Rc::clone(&cpu),
            font: ttf_context.load_font("debug.ttf", 16).unwrap(),
            texture_creator
        };

        let mut disasm_string: String;
        let mut current_addr: u16 = begin_address;
        let mut on_screen_index: usize = 0;
        let mut last_offset: u16;

        (disasm_string, last_offset) = disasm_6502(current_addr, &cpu.borrow().memory);
        result.disasm.insert(current_addr, format!("${:X}: {}", current_addr, disasm_string));
        result.addresses[on_screen_index] = current_addr;
        on_screen_index += 1;
        current_addr += last_offset;

        for _ in 0..19 {
            (disasm_string, last_offset) = disasm_6502(current_addr, &cpu.borrow().memory);
            result.disasm.insert(current_addr, format!("${:X}: {}", current_addr, disasm_string));
            result.addresses[on_screen_index] = current_addr;
            on_screen_index += 1;
            current_addr += last_offset;
        }

        result
    }

    pub fn render(&self, mut canvas: RefMut<Canvas<Window>>) {
        let surface = self.font
            .render(
            self.addresses.iter().enumerate()
                .map(|i| {
                    if i.0 == self.selected_entry {
                        "> ".to_owned() + self.disasm[i.1].as_str()
                    } else {
                        "  ".to_owned() + self.disasm[i.1].as_str()
                    }
                }).collect::<Vec<String>>().join("\n").as_str()
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
    }
}