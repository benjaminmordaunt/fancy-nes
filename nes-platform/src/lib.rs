extern crate sdl2;

pub const NES_SCREEN_SCALE: u32 = 2;
pub const NES_SCREEN_HEIGHT: u32 = 240 * NES_SCREEN_SCALE;
pub const NES_SCREEN_WIDTH: u32 = 256 * NES_SCREEN_SCALE;
pub const NES_DEBUGGER_WIDTH: u32 = 260;
pub const NES_PPU_INFO_HEIGHT: u32 = 200;

pub mod debug_view;

use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::Rect;
use sdl2::render::TextureQuery;
use std::path::PathBuf;
use std::time::Duration;

pub fn render_main() {
    let mut disasm_strings = ["TEST", "APPLE"].iter().map(|s| s.to_string()).collect::<Vec<String>>();
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

    let font = ttf_context.load_font("debug.ttf", 22).unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();

    disasm_strings.iter_mut().enumerate()
            .for_each(|i| {
                if i.0 == disasm_sel {
                    *i.1 = "> ".to_owned() + i.1;
                } else {
                    *i.1 = "  ".to_owned() + i.1;
                }
             });

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit {..} |
                Event::KeyDown { keycode: Some(Keycode::Escape), ..} => {
                    break 'running
                },
                _ => {}
            }
        }

        let surface = font
            .render(disasm_strings.join("\n").as_str())
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

pub fn load_palette(colors: PathBuf) -> Vec<Color> {
    let mut color_vec: Vec<Color> = vec![];

    let data: Vec<u8> = std::fs::read(colors).unwrap();
    data.chunks(3).for_each(|c| { color_vec.push(Color::RGB(c[0], c[1], c[2])) });
    color_vec
}