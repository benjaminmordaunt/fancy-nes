extern crate sdl2;

use sdl2::pixels::Color;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::rect::Rect;
use sdl2::render::TextureQuery;
use std::time::Duration;

pub fn render_main() {
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();

    let window = video_subsystem.window("fancy-nes v0.1.0", 256 * 2 + 180, 240 * 2)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    let mut font = ttf_context.load_font("debug.ttf", 18).unwrap();

    let surface = font
        .render("N V - B D I Z C")
        .blended(Color::RGBA(255, 255, 255, 255))
        .map_err(|e| e.to_string()).unwrap();

    font.set_style(sdl2::ttf::FontStyle::BOLD);

    let texture = texture_creator
        .create_texture_from_surface(&surface)
        .map_err(|e| e.to_string()).unwrap();

    canvas.set_draw_color(Color::RGBA(195, 217, 255, 255));
    canvas.clear();

    let TextureQuery { width, height, .. } = texture.query();

    let text_rect = Rect::new(256*2+10, 10, width, height);

    canvas.set_draw_color(Color::RGBA(0, 0, 255, 180));
    canvas.fill_rect(Rect::new(256*2, 0, 180, 240*2)).unwrap();

    canvas.copy(&texture, None, Some(text_rect)).unwrap();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();
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
    }
}