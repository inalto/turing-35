//! RGB canvas with text drawing and dirty-rect flushing for the Turing 3.5" display.
//!
//! Draw into a [`Canvas`] with [`Canvas::fill`], [`Canvas::fill_rect`], [`Canvas::blend_rect`],
//! [`Canvas::draw_line`] (pixels from [`line_points`]), [`Canvas::draw_rect_outline`], [`Canvas::blit_from`],
//! [`Canvas::draw_rgb`] and [`Canvas::draw_text`], then call [`Canvas::flush`]
//! to send only what changed since the previous flush.

use std::io::Write;

use fontdue::layout::{CoordinateSystem, Layout, LayoutSettings, TextStyle};
use turing_device::Display;
use turing_proto::{encode_rgb, Rect};

/// When the changed area exceeds this fraction of the panel, one full-frame
/// write is cheaper than many partial ones.
pub const FULL_REDRAW_FRACTION: f32 = 0.4;

/// 8-bit RGB colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
    pub const BLACK: Color = Color::rgb(0, 0, 0);
    pub const WHITE: Color = Color::rgb(255, 255, 255);
}

/// A TrueType/OpenType font usable by [`Canvas::draw_text`].
pub struct Font {
    inner: fontdue::Font,
}

/// Roboto Regular, bundled (Apache 2.0, see `fonts/LICENSE-Roboto.txt`).
pub const DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../fonts/Roboto-Regular.ttf");

impl Font {
    /// Parse a font file. Returns the parser's message on failure.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, String> {
        fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default())
            .map(|inner| Self { inner })
            .map_err(|e| e.to_string())
    }

    /// The bundled Roboto Regular.
    pub fn default_font() -> Self {
        Self::from_bytes(DEFAULT_FONT_BYTES).expect("bundled font is valid")
    }

    fn line_height(&self, px: f32) -> f32 {
        self.inner.horizontal_line_metrics(px).map(|m| m.new_line_size).unwrap_or(px)
    }
}

/// Measure `text` at `px` pixels: `(width, line height)` in whole pixels.
pub fn measure_text(text: &str, font: &Font, px: f32) -> (u16, u16) {
    let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
    layout.append(&[&font.inner], &TextStyle::new(text, px, 0));
    let right = layout
        .glyphs()
        .iter()
        .map(|g| g.x + g.width as f32)
        .fold(0.0f32, f32::max);
    (right.ceil() as u16, font.line_height(px).ceil() as u16)
}

/// The pixels of the Bresenham line from `(x0, y0)` to `(x1, y1)`, endpoints
/// included, in order from start to end, unclipped. [`Canvas::draw_line`]
/// paints exactly these pixels, so callers can decorate a line (e.g. a glow)
/// pixel for pixel.
pub fn line_points(x0: i32, y0: i32, x1: i32, y1: i32) -> impl Iterator<Item = (i32, i32)> {
    let (x1, y1) = (x1 as i64, y1 as i64);
    let (mut x, mut y) = (x0 as i64, y0 as i64);
    let dx = (x1 - x).abs();
    let dy = -(y1 - y).abs();
    let sx = if x < x1 { 1 } else { -1 };
    let sy = if y < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut done = false;
    std::iter::from_fn(move || {
        if done {
            return None;
        }
        // Every point lies between the endpoints, so it fits back into i32.
        let point = (x as i32, y as i32);
        if x == x1 && y == y1 {
            done = true;
        } else {
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
        Some(point)
    })
}

/// An RGB framebuffer plus the RGB565 image last sent to the panel.
pub struct Canvas {
    w: u16,
    h: u16,
    rgb: Vec<u8>,
    last_sent: Option<Vec<u8>>,
}

impl Canvas {
    /// A black canvas of `w`×`h` pixels.
    pub fn new(w: u16, h: u16) -> Self {
        Self { w, h, rgb: vec![0; w as usize * h as usize * 3], last_sent: None }
    }

    /// 480×320, the 3.5" panel in landscape.
    pub fn landscape() -> Self {
        Self::new(480, 320)
    }

    pub fn width(&self) -> u16 {
        self.w
    }

    pub fn height(&self) -> u16 {
        self.h
    }

    /// Packed 8-bit RGB, row-major.
    pub fn as_rgb(&self) -> &[u8] {
        &self.rgb
    }

    pub fn pixel(&self, x: u16, y: u16) -> Color {
        let i = self.index(x, y);
        Color::rgb(self.rgb[i], self.rgb[i + 1], self.rgb[i + 2])
    }

    fn index(&self, x: u16, y: u16) -> usize {
        (y as usize * self.w as usize + x as usize) * 3
    }

    fn put(&mut self, x: u16, y: u16, c: Color) {
        let i = self.index(x, y);
        self.rgb[i] = c.r;
        self.rgb[i + 1] = c.g;
        self.rgb[i + 2] = c.b;
    }

    fn blend(&mut self, x: u16, y: u16, c: Color, alpha: u8) {
        if alpha == 255 {
            return self.put(x, y, c);
        }
        let i = self.index(x, y);
        let a = alpha as u32;
        for (k, v) in [c.r, c.g, c.b].into_iter().enumerate() {
            let d = self.rgb[i + k] as u32;
            self.rgb[i + k] = ((v as u32 * a + d * (255 - a)) / 255) as u8;
        }
    }

    /// Clip `rect` to the canvas; `None` when nothing is left.
    fn clip(&self, rect: Rect) -> Option<Rect> {
        if rect.x >= self.w || rect.y >= self.h {
            return None;
        }
        let w = rect.w.min(self.w - rect.x);
        let h = rect.h.min(self.h - rect.y);
        (w > 0 && h > 0).then_some(Rect { x: rect.x, y: rect.y, w, h })
    }

    pub fn fill(&mut self, c: Color) {
        for px in self.rgb.chunks_exact_mut(3) {
            px.copy_from_slice(&[c.r, c.g, c.b]);
        }
    }

    /// Fill `rect` (clipped to the canvas).
    pub fn fill_rect(&mut self, rect: Rect, c: Color) {
        if let Some(r) = self.clip(rect) {
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    self.put(x, y, c);
                }
            }
        }
    }

    /// Alpha-blend `c` over every pixel of `rect` (clipped to the canvas).
    /// `alpha` 0 leaves the canvas untouched, 255 is equivalent to [`Canvas::fill_rect`].
    pub fn blend_rect(&mut self, rect: Rect, c: Color, alpha: u8) {
        if let Some(r) = self.clip(rect) {
            for y in r.y..r.y + r.h {
                for x in r.x..r.x + r.w {
                    self.blend(x, y, c, alpha);
                }
            }
        }
    }

    /// Draw a 1-px line from `(x0, y0)` to `(x1, y1)`, endpoints inclusive,
    /// through the pixels of [`line_points`]. Coordinates may lie outside the
    /// canvas; each pixel is clipped.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color) {
        for (x, y) in line_points(x0, y0, x1, y1) {
            if (0..self.w as i32).contains(&x) && (0..self.h as i32).contains(&y) {
                self.put(x as u16, y as u16, c);
            }
        }
    }

    /// Draw a 1-px outline just inside `rect`; edges that fall outside the
    /// canvas are clipped away.
    pub fn draw_rect_outline(&mut self, rect: Rect, c: Color) {
        if rect.w == 0 || rect.h == 0 {
            return;
        }
        // Far edges are computed in u32 and skipped when beyond u16 (hence off-canvas).
        let right = u16::try_from(rect.x as u32 + rect.w as u32 - 1).ok();
        let bottom = u16::try_from(rect.y as u32 + rect.h as u32 - 1).ok();
        self.fill_rect(Rect { x: rect.x, y: rect.y, w: rect.w, h: 1 }, c);
        self.fill_rect(Rect { x: rect.x, y: rect.y, w: 1, h: rect.h }, c);
        if let Some(y) = bottom {
            self.fill_rect(Rect { x: rect.x, y, w: rect.w, h: 1 }, c);
        }
        if let Some(x) = right {
            self.fill_rect(Rect { x, y: rect.y, w: 1, h: rect.h }, c);
        }
    }

    /// Copy `rect` (clipped to the canvas) from `src`, which must be the same
    /// size as `self`. Panics otherwise.
    pub fn blit_from(&mut self, src: &Canvas, rect: Rect) {
        assert_eq!(
            (self.w, self.h),
            (src.w, src.h),
            "blit_from: source and destination canvases must be the same size"
        );
        let Some(r) = self.clip(rect) else { return };
        for y in r.y..r.y + r.h {
            let i = self.index(r.x, y);
            let n = r.w as usize * 3;
            self.rgb[i..i + n].copy_from_slice(&src.rgb[i..i + n]);
        }
    }

    /// Copy a packed 8-bit RGB image of `w`×`h` to `(x, y)`, clipping any overhang.
    pub fn draw_rgb(&mut self, x: u16, y: u16, w: u16, h: u16, rgb: &[u8]) {
        assert_eq!(rgb.len(), w as usize * h as usize * 3, "image buffer size");
        let Some(r) = self.clip(Rect { x, y, w, h }) else { return };
        for row in 0..r.h {
            let src = ((row as usize) * w as usize) * 3;
            let dst = self.index(r.x, r.y + row);
            let n = r.w as usize * 3;
            self.rgb[dst..dst + n].copy_from_slice(&rgb[src..src + n]);
        }
    }

    /// Draw `text` with its top-left at `(x, y)`; returns the clipped bounding
    /// box of the pixels that were painted (zero-sized if nothing was).
    pub fn draw_text(&mut self, x: u16, y: u16, text: &str, font: &Font, px: f32, c: Color) -> Rect {
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings { x: x as f32, y: y as f32, ..LayoutSettings::default() });
        layout.append(&[&font.inner], &TextStyle::new(text, px, 0));

        let (mut x0, mut y0, mut x1, mut y1) = (u16::MAX, u16::MAX, 0u16, 0u16);
        for g in layout.glyphs() {
            if g.width == 0 || g.height == 0 {
                continue;
            }
            let (_, bitmap) = font.inner.rasterize_config(g.key);
            let gx = g.x.round().max(0.0) as i64;
            let gy = g.y.round().max(0.0) as i64;
            for row in 0..g.height {
                let py = gy + row as i64;
                if py >= self.h as i64 {
                    break;
                }
                for col in 0..g.width {
                    let pxx = gx + col as i64;
                    if pxx >= self.w as i64 {
                        break;
                    }
                    let a = bitmap[row * g.width + col];
                    if a == 0 {
                        continue;
                    }
                    let (cx, cy) = (pxx as u16, py as u16);
                    self.blend(cx, cy, c, a);
                    x0 = x0.min(cx);
                    y0 = y0.min(cy);
                    x1 = x1.max(cx);
                    y1 = y1.max(cy);
                }
            }
        }
        if x0 == u16::MAX {
            return Rect { x, y, w: 0, h: 0 };
        }
        Rect { x: x0, y: y0, w: x1 - x0 + 1, h: y1 - y0 + 1 }
    }

    /// The whole canvas as RGB565 little-endian bytes.
    fn encode(&self) -> Vec<u8> {
        let mut out = vec![0u8; self.w as usize * self.h as usize * 2];
        encode_rgb(&self.rgb, &mut out);
        out
    }

    /// Rows that differ between `old` and `new`, grouped into bands of
    /// consecutive rows; each band spans the union of its changed columns.
    fn dirty_bands(&self, old: &[u8], new: &[u8]) -> Vec<Rect> {
        let stride = self.w as usize * 2;
        let mut bands: Vec<Rect> = Vec::new();
        for y in 0..self.h as usize {
            let (o, n) = (&old[y * stride..(y + 1) * stride], &new[y * stride..(y + 1) * stride]);
            let changed: Vec<usize> = (0..self.w as usize)
                .filter(|&x| o[x * 2..x * 2 + 2] != n[x * 2..x * 2 + 2])
                .collect();
            let (Some(&x0), Some(&x1)) = (changed.first(), changed.last()) else { continue };
            let (x0, x1) = (x0 as u16, x1 as u16);
            match bands.last_mut() {
                Some(b) if b.y + b.h == y as u16 => {
                    let right = (b.x + b.w).max(x1 + 1);
                    b.x = b.x.min(x0);
                    b.w = right - b.x;
                    b.h += 1;
                }
                _ => bands.push(Rect { x: x0, y: y as u16, w: x1 - x0 + 1, h: 1 }),
            }
        }
        bands
    }

    /// Send what changed since the last flush. The first flush sends the full
    /// frame; afterwards only dirty row bands are sent, unless they cover more
    /// than [`FULL_REDRAW_FRACTION`] of the panel. Returns the rects written.
    pub fn flush<W: Write>(&mut self, display: &mut Display<W>) -> turing_device::Result<Vec<Rect>> {
        let full = Rect { x: 0, y: 0, w: self.w, h: self.h };
        let (width, height) = display.size();
        if (width, height) != (self.w, self.h) {
            return Err(turing_device::Error::OutOfBounds { rect: full, width, height });
        }
        let new = self.encode();
        let rects = match &self.last_sent {
            None => vec![full],
            Some(old) => {
                let bands = self.dirty_bands(old, &new);
                let area: usize = bands.iter().map(|r| r.w as usize * r.h as usize).sum();
                if area as f32 > FULL_REDRAW_FRACTION * (self.w as f32 * self.h as f32) {
                    vec![full]
                } else {
                    bands
                }
            }
        };
        for &r in &rects {
            let px = self.crop(&new, r);
            display.blit(r, &px)?;
        }
        if !rects.is_empty() {
            self.last_sent = Some(new);
        }
        Ok(rects)
    }

    /// Forget what was last sent, so the next [`Canvas::flush`] sends the full
    /// frame. Use after the panel lost its contents (e.g. a reconnect).
    pub fn invalidate(&mut self) {
        self.last_sent = None;
    }

    /// Rows of `r` out of a full-frame RGB565 buffer.
    fn crop(&self, frame: &[u8], r: Rect) -> Vec<u8> {
        let stride = self.w as usize * 2;
        let mut out = Vec::with_capacity(r.byte_len());
        for y in r.y as usize..(r.y + r.h) as usize {
            let start = y * stride + r.x as usize * 2;
            out.extend_from_slice(&frame[start..start + r.w as usize * 2]);
        }
        out
    }
}
