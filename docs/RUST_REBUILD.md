# Rust rebuild — proposed architecture

Goal: a cross-platform daemon that owns the Turing 3.5" panel, renders frames, and exposes an HTTP
API so anything (scripts, home automation, a web page) can push content to the display.
Everything below the API is derived from `PROTOCOL.md`; nothing depends on the vendor app.

## 1. Workspace layout

```
turing-screen/
├─ Cargo.toml                  # workspace
├─ crates/
│  ├─ turing-proto/            # pure, no_std-friendly protocol: header packing, command builders,
│  │                           # RGB565 conversion, rect math.  Unit-tested against PROTOCOL.md vectors.
│  ├─ turing-device/           # discovery (VID 0x1A86 + serial contains "USB35INCH"), serialport
│  │                           # transport, write lock, reconnect, pacing (10 ms gaps on non-V2).
│  ├─ turing-canvas/           # 480x320 RGBA canvas (tiny-skia) + text (cosmic-text or fontdue),
│  │                           # dirty-rect tracking, RGB565 LE encoder, diff against last frame.
│  └─ turing-daemon/           # binary: axum HTTP API, scene/widget scheduler, config, sensors.
└─ docs/
```

Suggested dependencies: `serialport` (enumeration gives `UsbPortInfo { vid, pid, serial_number }`),
`tiny-skia`, `cosmic-text` (or `ab_glyph`), `image` (decode uploads), `axum` + `tokio`,
`serde`/`serde_json`, `tracing`, `sysinfo` and `nvml-wrapper` for the monitoring widgets later.

## 2. `turing-proto` (core, ~200 lines)

```rust
#[repr(u8)]
pub enum Cmd { Reset = 101, Clear = 102, ToBlack = 103, ScreenOff = 108, ScreenOn = 109,
               Brightness = 110, Orientation = 121, Mirror = 122, Bitmap = 197,
               BitmapNoWait = 198, DrawPixels = 195, Probe = 255 }

#[derive(Clone, Copy)] pub enum Orientation { Portrait = 0, PortraitFlipped = 1, Landscape = 2, LandscapeFlipped = 3 }

pub fn header(cmd: Cmd, x: u16, y: u16, ex: u16, ey: u16) -> [u8; 6] {
    [ (x >> 2) as u8,
      (((x & 3) << 6) | (y >> 4)) as u8,
      (((y & 15) << 4) | (ex >> 6)) as u8,
      (((ex & 63) << 2) | (ey >> 8)) as u8,
      (ey & 0xFF) as u8,
      cmd as u8 ]
}

pub fn orientation_packet(o: Orientation, w: u16, h: u16) -> [u8; 16] {
    let mut p = [0u8; 16];
    p[..6].copy_from_slice(&header(Cmd::Orientation, 0, 0, 0, 0));
    p[6] = 100 + o as u8; p[7..9].copy_from_slice(&w.to_be_bytes()); p[9..11].copy_from_slice(&h.to_be_bytes());
    p
}

pub fn bitmap_header(x: u16, y: u16, w: u16, h: u16) -> [u8; 6] { header(Cmd::Bitmap, x, y, x + w - 1, y + h - 1) }

#[inline] pub fn rgb565(r: u8, g: u8, b: u8) -> u16 { ((r as u16 & 0xF8) << 8) | ((g as u16 & 0xFC) << 3) | (b as u16 >> 3) }
// encode rows into little-endian u16s: buf.extend_from_slice(&px.to_le_bytes())
```

Test vectors (from PROTOCOL.md §2): `(197,0,0,479,319) → 00 00 07 7D 3F C5`, `(197,20,20,119,69) → 05 01 41 DC 45 C5`.

## 3. `turing-device`

* `Display::discover() -> Result<Vec<PortCandidate>>` — filter `serialport::available_ports()` by
  `UsbPort { vid: 0x1A86, serial_number contains "USB35INCH" }`; fall back to `pid == 0xF111`.
* `Display::open(path)` — 115200 8N1, DTR+RTS on, write timeout 5 s. Keep a `Mutex<Box<dyn SerialPort>>`.
* `fn send(&self, hdr: &[u8;6], data: Option<&[u8]>)` — one lock for header **and** data (fixes the
  vendor's non-atomic pair). Optional `pace: Duration` (10 ms) when the serial string lacks `V2`.
* High-level: `screen_on/off`, `brightness(u8)`, `orientation(o)`, `blit(rect, &[u8])`, `clear`, `probe`.
* Reconnect loop: on write error, drop the port, re-discover every 2 s, replay `screen_on → orientation → brightness → full frame`.
* Note: a full frame is ~2.4 s on the wire. Never block the API thread on it; push to a dedicated
  render thread via a bounded channel and coalesce (latest frame wins).

## 4. `turing-canvas`

* `Canvas { w: 480, h: 320, rgba: Vec<u8>, last_sent: Vec<u16>, dirty: Vec<Rect> }`.
* Drawing API: `fill`, `image(x, y, &DynamicImage)`, `text(x, y, &Style, &str)`, `bar`, `arc` — all CPU, no GPU.
* `flush(&mut self, dev: &Display)`: encode only dirty rects (merge overlapping, cap to N rects per
  flush), or the whole frame if dirty area > ~40 %. This reproduces what the vendor does with its
  per-item glyph boxes but generically: diff `rgb565(new)` vs `last_sent` and send bounding boxes of changed rows.
* Rotation is done by the panel (cmd 121); the canvas is always logical landscape or portrait size.

## 5. `turing-daemon` HTTP API (axum)

```
GET  /v1/device                 -> { port, serial, connected, orientation, brightness, size }
POST /v1/device/power           { "on": true }
POST /v1/device/brightness      { "level": 0..255 }     # 0 = brightest (vendor convention) — API can expose 0..100 and invert
POST /v1/device/orientation     { "orientation": "landscape" | "portrait" | ... }
POST /v1/display/clear          { "color": "#000000" }
POST /v1/display/image          multipart or body = PNG/JPEG/BMP; query ?x=&y=&fit=contain|cover|stretch
POST /v1/display/raw            body = w*h*2 RGB565 LE bytes, headers X-Rect: x,y,w,h   # zero-copy path
POST /v1/display/text           { "x","y","text","font","size","color","bg" }
POST /v1/scene                  { "widgets":[ {"type":"text","bind":"cpu.temp", ...}, {"type":"bar",...} ] }
                                 # persistent layout the daemon re-renders itself every N ms
GET  /v1/scene / DELETE /v1/scene
WS   /v1/stream                 # optional: push raw frames continuously (screen mirroring)
```

Internals: `axum` handlers only mutate a shared `Scene`/`Canvas` behind a `Mutex` and send a
`RenderRequest` on a channel; a single render task owns the `Display`. Responses are immediate; the
device state endpoint reports the last successful flush. Add `--port COMx` override and `--list`.

## 6. Mapping vendor features → Rust (optional, later)

| Vendor | Rust |
|---|---|
| Themes (`.data`, BinaryFormatter) | JSON scene files; a one-off converter can read `.data` with a .NET script if desired |
| Sensors (WMI, NVML, AIDA64) | `sysinfo`, `nvml-wrapper`; Linux gets `/sys` for free |
| Weather | leave to the API client; daemon stays dumb |
| Arc gauges via cmd 195 | draw with tiny-skia and blit the bounding box; 195 is unnecessary |
| Auto-run | OS service / systemd unit |

## 7. First milestone

1. `turing-proto` with unit tests for the header vectors and RGB565.
2. `turing-device` + `examples/show_png.rs` — reproduce today's live test (`tools/make_frame.py`) in Rust.
3. `turing-daemon` with `/v1/display/image`, `/v1/device/*`.
4. Dirty-rect flushing and `/v1/scene` with a text widget bound to a system value.
