//! Wire protocol for the Turing 3.5" USB display (`USB35INCHIPSV2`).
//!
//! Every command is a 6-byte header. Pixel commands are followed by raw RGB565
//! little-endian pixels in a second write. Reverse-engineered from the vendor
//! `UsbMonitor.exe` and verified on hardware; see `docs/PROTOCOL.md`.
#![no_std]

/// Command ids as sent in header byte 5.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cmd {
    Reset = 101,
    Clear = 102,
    ToBlack = 103,
    ScreenOff = 108,
    ScreenOn = 109,
    /// `x` field carries the level: 0 = brightest, 255 = darkest.
    Brightness = 110,
    /// 16-byte packet, see [`orientation`].
    Orientation = 121,
    /// 16-byte packet, see [`mirror`].
    Mirror = 122,
    /// Header with inclusive rect, then `w*h*2` bytes RGB565 LE.
    Bitmap = 197,
    /// Same as [`Cmd::Bitmap`]; the vendor sends it without inter-packet sleeps.
    BitmapNoWait = 198,
    /// 64-byte packet: header, RGB565 colour (BE), then up to 56 bytes of `(dx, dy)` pairs.
    DrawPixels = 195,
    /// Vendor "test connect": a successful write means connected. No reply.
    Probe = 255,
}

/// Panel orientation. The byte on the wire is `100 + value`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    Portrait = 0,
    PortraitFlipped = 1,
    Landscape = 2,
    LandscapeFlipped = 3,
}

impl Orientation {
    /// Logical framebuffer size `(width, height)` for this orientation on the 3.5" panel.
    pub const fn size(self) -> (u16, u16) {
        match self {
            Orientation::Portrait | Orientation::PortraitFlipped => (320, 480),
            Orientation::Landscape | Orientation::LandscapeFlipped => (480, 320),
        }
    }
}

/// A rectangle in logical pixels (`x`, `y` top-left, `w`, `h` extent).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
}

impl Rect {
    /// Number of RGB565 bytes a bitmap of this rect needs.
    pub const fn byte_len(self) -> usize {
        self.w as usize * self.h as usize * 2
    }
}

/// Pack the 6-byte command header exactly as the vendor app does.
///
/// `x`, `y`, `ex` are 10-bit, `ey` is 9-bit plus its low byte; all are
/// inclusive panel coordinates for pixel commands.
pub const fn header(cmd: Cmd, x: u16, y: u16, ex: u16, ey: u16) -> [u8; 6] {
    [
        (x >> 2) as u8,
        (((x & 0x03) << 6) | (y >> 4)) as u8,
        (((y & 0x0F) << 4) | (ex >> 6)) as u8,
        (((ex & 0x3F) << 2) | (ey >> 8)) as u8,
        (ey & 0xFF) as u8,
        cmd as u8,
    ]
}

const fn control(cmd: Cmd) -> [u8; 6] {
    header(cmd, 0, 0, 0, 0)
}

pub const fn reset() -> [u8; 6] {
    control(Cmd::Reset)
}
pub const fn clear() -> [u8; 6] {
    control(Cmd::Clear)
}
pub const fn to_black() -> [u8; 6] {
    control(Cmd::ToBlack)
}
pub const fn screen_off() -> [u8; 6] {
    control(Cmd::ScreenOff)
}
pub const fn screen_on() -> [u8; 6] {
    control(Cmd::ScreenOn)
}
pub const fn probe() -> [u8; 6] {
    control(Cmd::Probe)
}

/// Brightness: `0` is brightest, `255` darkest (vendor convention).
pub const fn brightness(level: u8) -> [u8; 6] {
    header(Cmd::Brightness, level as u16, 0, 0, 0)
}

/// Orientation + logical size packet (16 bytes).
pub const fn orientation(o: Orientation, width: u16, height: u16) -> [u8; 16] {
    let h = header(Cmd::Orientation, 0, 0, 0, 0);
    [
        h[0], h[1], h[2], h[3], h[4], h[5],
        100 + o as u8,
        (width >> 8) as u8, (width & 0xFF) as u8,
        (height >> 8) as u8, (height & 0xFF) as u8,
        0, 0, 0, 0, 0,
    ]
}

/// Mirror flag packet (16 bytes).
pub const fn mirror(on: bool) -> [u8; 16] {
    let h = header(Cmd::Mirror, 0, 0, 0, 0);
    [h[0], h[1], h[2], h[3], h[4], h[5], on as u8, 0, 0, 0, 0, 0, 0, 0, 0, 0]
}

/// Header for a [`Cmd::Bitmap`] write covering `rect`; follow it with `rect.byte_len()` bytes.
pub const fn bitmap_header(rect: Rect) -> [u8; 6] {
    header(Cmd::Bitmap, rect.x, rect.y, rect.x + rect.w - 1, rect.y + rect.h - 1)
}

/// Pack 8-bit RGB into RGB565 (`RRRRRGGG GGGBBBBB`).
#[inline]
pub const fn rgb565(r: u8, g: u8, b: u8) -> u16 {
    ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3)
}

/// Convert packed 8-bit RGB triples into little-endian RGB565 bytes.
///
/// `out` must hold `rgb.len() / 3 * 2` bytes; extra bytes are left untouched.
pub fn encode_rgb(rgb: &[u8], out: &mut [u8]) {
    for (px, dst) in rgb.chunks_exact(3).zip(out.chunks_exact_mut(2)) {
        dst.copy_from_slice(&rgb565(px[0], px[1], px[2]).to_le_bytes());
    }
}
