//! Serial transport and discovery for the Turing 3.5" USB display.
//!
//! [`Display`] is generic over any [`std::io::Write`] sink so the byte stream
//! can be tested in memory; [`Display::open`] binds it to a real serial port.
//! All numbers and sequences come from `docs/PROTOCOL.md`.

use std::io::Write;
use std::thread;
use std::time::Duration;

use serialport::{SerialPort, SerialPortType};
use turing_proto::{bitmap_header, brightness, encode_rgb, orientation, screen_off, screen_on, Orientation, Rect};

/// USB vendor id of the WCH bridge used by every known unit.
pub const USB_VID: u16 = 0x1A86;
/// Product id of the legacy units that lack the `USB35INCH` serial string.
pub const LEGACY_PID: u16 = 0xF111;
/// Substring of the USB serial string on current units (`USB35INCHIPSV2`).
pub const SERIAL_MARKER: &str = "USB35INCH";
/// Gap the vendor app leaves around pixel data on pre-V2 firmware.
pub const LEGACY_PACING: Duration = Duration::from_millis(10);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("pixel buffer is {got} bytes, rect needs {expected}")]
    SizeMismatch { expected: usize, got: usize },
    #[error("rect {rect:?} does not fit in {width}x{height}")]
    OutOfBounds { rect: Rect, width: u16, height: u16 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Serial(#[from] serialport::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether a USB serial port looks like a Turing 3.5" display.
pub fn is_turing_port(vid: u16, pid: u16, serial: Option<&str>) -> bool {
    vid == USB_VID && (pid == LEGACY_PID || serial.is_some_and(|s| s.contains(SERIAL_MARKER)))
}

/// Delay to insert after the bitmap header and after pixel data, if any.
/// Units whose serial string contains `V2` take back-to-back writes.
pub fn pacing_for(serial: Option<&str>) -> Option<Duration> {
    match serial {
        Some(s) if s.contains("V2") => None,
        _ => Some(LEGACY_PACING),
    }
}

/// A serial port that passed [`is_turing_port`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    /// OS port name, e.g. `COM20` or `/dev/ttyACM0`.
    pub port: String,
    pub pid: u16,
    pub serial: Option<String>,
}

/// Enumerate serial ports and keep the ones that look like a Turing display.
pub fn discover() -> Result<Vec<Candidate>> {
    let mut out = Vec::new();
    for p in serialport::available_ports()? {
        if let SerialPortType::UsbPort(u) = p.port_type {
            if is_turing_port(u.vid, u.pid, u.serial_number.as_deref()) {
                out.push(Candidate { port: p.port_name, pid: u.pid, serial: u.serial_number });
            }
        }
    }
    Ok(out)
}

/// A connected display: tracks logical size and serializes all writes.
pub struct Display<W: Write> {
    sink: W,
    pacing: Option<Duration>,
    size: (u16, u16),
}

impl Display<Box<dyn SerialPort>> {
    /// Open `port` with the vendor's settings: 115200 8N1, DTR and RTS asserted.
    /// Pacing is chosen from the port's USB serial string when it can be found.
    pub fn open(port: &str) -> Result<Self> {
        let serial = discover()?.into_iter().find(|c| c.port == port).and_then(|c| c.serial);
        let mut sp = serialport::new(port, 115_200)
            .data_bits(serialport::DataBits::Eight)
            .parity(serialport::Parity::None)
            .stop_bits(serialport::StopBits::One)
            .flow_control(serialport::FlowControl::None)
            .timeout(Duration::from_secs(5))
            .open()?;
        sp.write_data_terminal_ready(true)?;
        sp.write_request_to_send(true)?;
        Ok(Self::new(sp, pacing_for(serial.as_deref())))
    }
}

impl<W: Write> Display<W> {
    /// Wrap an already-open sink. The logical size starts as landscape 480x320.
    pub fn new(sink: W, pacing: Option<Duration>) -> Self {
        Self { sink, pacing, size: Orientation::Landscape.size() }
    }

    /// Current logical `(width, height)`.
    pub fn size(&self) -> (u16, u16) {
        self.size
    }

    /// Borrow the underlying sink (useful for tests and raw access).
    pub fn sink(&self) -> &W {
        &self.sink
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        self.sink.write_all(bytes)?;
        Ok(())
    }

    fn pace(&self) {
        if let Some(d) = self.pacing {
            thread::sleep(d);
        }
    }

    /// Vendor wake sequence: screen on, orientation + size, brightness.
    pub fn init(&mut self, o: Orientation, level: u8) -> Result<()> {
        self.screen_on()?;
        self.set_orientation(o)?;
        self.set_brightness(level)
    }

    pub fn screen_on(&mut self) -> Result<()> {
        self.write(&screen_on())
    }

    pub fn screen_off(&mut self) -> Result<()> {
        self.write(&screen_off())
    }

    /// `0` is brightest, `255` darkest.
    pub fn set_brightness(&mut self, level: u8) -> Result<()> {
        self.write(&brightness(level))
    }

    pub fn set_orientation(&mut self, o: Orientation) -> Result<()> {
        let (w, h) = o.size();
        self.write(&orientation(o, w, h))?;
        self.size = (w, h);
        Ok(())
    }

    /// Send RGB565 little-endian pixels for `rect` (`rect.byte_len()` bytes).
    pub fn blit(&mut self, rect: Rect, pixels: &[u8]) -> Result<()> {
        let (width, height) = self.size;
        if rect.w == 0 || rect.h == 0 || rect.x + rect.w > width || rect.y + rect.h > height {
            return Err(Error::OutOfBounds { rect, width, height });
        }
        if pixels.len() != rect.byte_len() {
            return Err(Error::SizeMismatch { expected: rect.byte_len(), got: pixels.len() });
        }
        self.write(&bitmap_header(rect))?;
        self.pace();
        self.write(pixels)?;
        self.pace();
        Ok(())
    }

    /// Convert packed 8-bit RGB (`rect.w * rect.h * 3` bytes) and blit it.
    pub fn show_rgb(&mut self, rect: Rect, rgb: &[u8]) -> Result<()> {
        let expected = rect.w as usize * rect.h as usize * 3;
        if rgb.len() != expected {
            return Err(Error::SizeMismatch { expected, got: rgb.len() });
        }
        let mut px = vec![0u8; rect.byte_len()];
        encode_rgb(rgb, &mut px);
        self.blit(rect, &px)
    }
}
