//! Show an image file on the Turing 3.5" display.
//!
//! ```text
//! cargo run -p turing-device --example show_png -- path/to/image.png [COMx]
//! ```
//! The image is scaled to fit 480x320 (landscape) preserving aspect ratio and
//! centred on black. Without a port argument the display is auto-discovered.
use std::env;
use turing_device::{discover, Display};
use turing_proto::{Orientation, Rect};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("usage: show_png <image> [COMx]")?;
    let port = match args.next() {
        Some(p) => p,
        None => {
            let found = discover()?;
            let first = found.first().ok_or("no Turing display found")?;
            eprintln!("found {} ({})", first.port, first.serial.as_deref().unwrap_or("?"));
            first.port.clone()
        }
    };

    let mut display = Display::open(&port)?;
    display.init(Orientation::Landscape, 0)?;
    let (w, h) = display.size();

    let img = image::open(&path)?.to_rgb8();
    let scaled = image::imageops::resize(
        &img,
        scale_to_fit(img.width(), img.height(), w as u32, h as u32).0,
        scale_to_fit(img.width(), img.height(), w as u32, h as u32).1,
        image::imageops::FilterType::Triangle,
    );
    let mut canvas = image::RgbImage::new(w as u32, h as u32);
    let ox = (w as u32 - scaled.width()) / 2;
    let oy = (h as u32 - scaled.height()) / 2;
    image::imageops::replace(&mut canvas, &scaled, ox as i64, oy as i64);

    let t = std::time::Instant::now();
    display.show_rgb(Rect { x: 0, y: 0, w, h }, canvas.as_raw())?;
    eprintln!("sent {}x{} frame in {:?}", w, h, t.elapsed());
    Ok(())
}

fn scale_to_fit(iw: u32, ih: u32, mw: u32, mh: u32) -> (u32, u32) {
    let s = f64::min(mw as f64 / iw as f64, mh as f64 / ih as f64);
    (((iw as f64 * s).round() as u32).max(1), ((ih as f64 * s).round() as u32).max(1))
}
