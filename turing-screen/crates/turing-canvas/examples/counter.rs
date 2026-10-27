//! Live demo of dirty-rect flushing: draws a static background once, then
//! updates only a counter and an elapsed-time readout every second.
//!
//! ```text
//! cargo run -p turing-canvas --example counter -- [iterations] [COMx]
//! ```
use std::time::{Duration, Instant};

use turing_canvas::{Canvas, Color, Font};
use turing_device::{discover, Display};
use turing_proto::{Orientation, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let iterations: u32 = args.next().map(|s| s.parse()).transpose()?.unwrap_or(10);
    let port = match args.next() {
        Some(p) => p,
        None => discover()?.first().ok_or("no Turing display found")?.port.clone(),
    };
    let mut display = Display::open(&port)?;
    display.init(Orientation::Landscape, 0)?;

    let font = Font::default_font();
    let mut canvas = Canvas::landscape();
    canvas.fill(Color::rgb(16, 24, 48));
    canvas.fill_rect(Rect { x: 0, y: 0, w: 480, h: 56 }, Color::rgb(40, 90, 160));
    canvas.draw_text(16, 10, "turing-canvas dirty-rect demo", &font, 30.0, Color::WHITE);
    canvas.draw_text(16, 80, "counter", &font, 22.0, Color::rgb(160, 180, 210));
    canvas.draw_text(16, 190, "elapsed", &font, 22.0, Color::rgb(160, 180, 210));

    let t0 = Instant::now();
    let bg = Color::rgb(16, 24, 48);
    let mut counter_box = Rect { x: 16, y: 105, w: 300, h: 80 };
    let mut elapsed_box = Rect { x: 16, y: 215, w: 300, h: 80 };
    for i in 0..=iterations {
        canvas.fill_rect(counter_box, bg);
        canvas.fill_rect(elapsed_box, bg);
        counter_box = canvas.draw_text(16, 105, &format!("{i}"), &font, 72.0, Color::rgb(255, 200, 60));
        elapsed_box = canvas.draw_text(16, 215, &format!("{:.1} s", t0.elapsed().as_secs_f32()), &font, 56.0, Color::WHITE);
        let t = Instant::now();
        let sent = canvas.flush(&mut display)?;
        let bytes: usize = sent.iter().map(|r| 6 + r.byte_len()).sum();
        println!("flush {i}: {} rect(s), {bytes} bytes, {:?}  {:?}", sent.len(), t.elapsed(), sent);
        std::thread::sleep(Duration::from_secs(1));
    }
    Ok(())
}
