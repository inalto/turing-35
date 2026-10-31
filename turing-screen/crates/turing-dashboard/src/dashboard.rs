//! The dashboard: a static backdrop drawn once, per-tick rendering of a
//! [`Snapshot`] on top of it, and the sample → render → flush run loop with
//! reconnect.

use std::error::Error;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};
use turing_canvas::{Canvas, Color, Font};
use turing_device::{discover, Display};
use turing_proto::{Orientation, Rect};

use crate::telemetry::{fmt_gb, fmt_pct, fmt_rate, fmt_temp, Ring, Sampler, Snapshot};
use crate::theme::*;
use crate::widgets::{bar, fit_px, grow, sparkline, text_box, thread_bars, truncate_to_width};

/// Delay between reconnect attempts after a display error.
pub const RETRY: Duration = Duration::from_secs(2);

/// How long after a wall-clock second boundary an aligned tick wakes, so
/// the sampled time is safely in the new second.
pub const ALIGN_SLACK: Duration = Duration::from_millis(10);

/// Shortest refresh interval: sysinfo needs at least 200 ms between CPU
/// refreshes for valid loads.
pub const MIN_INTERVAL: Duration = Duration::from_millis(200);

/// `ms` milliseconds as a refresh interval, raised to [`MIN_INTERVAL`].
pub fn interval_from_ms(ms: u64) -> Duration {
    Duration::from_millis(ms).max(MIN_INTERVAL)
}

/// For an `interval` that is a whole number of seconds, the delay from
/// `now` (time since the Unix epoch) to the next tick: the first instant
/// strictly after `now` that is a multiple of `interval` since the epoch
/// plus [`ALIGN_SLACK`]. With a 1 s interval each tick samples the clock just
/// after a second boundary, so the displayed seconds never skip or repeat.
/// `None` for any other interval.
pub fn delay_to_aligned_tick(now: Duration, interval: Duration) -> Option<Duration> {
    let i = interval.as_nanos();
    if i == 0 || interval.subsec_nanos() != 0 {
        return None;
    }
    let (n, slack) = (now.as_nanos(), ALIGN_SLACK.as_nanos());
    let k = if n < slack { 0 } else { (n - slack) / i + 1 };
    Some(Duration::from_nanos((k * i + slack - n) as u64))
}

/// Wall-clock time since the Unix epoch (zero if the clock is before it).
fn wall_clock() -> Duration {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
}

/// Panel brightness sent on (re)connect: `0` is brightest.
const BRIGHTNESS: u8 = 0;

/// Dashboard state: the static backdrop, the canvas that is flushed to the
/// panel, and the network history.
pub struct Dashboard {
    backdrop: Canvas,
    /// The frame to flush; [`Dashboard::render`] redraws it.
    pub canvas: Canvas,
    font: Font,
    down: Ring,
    up: Ring,
    gpu_present: bool,
    /// Time of the last snapshot pushed into the rings, so rendering the same
    /// snapshot twice records one history sample and draws the same frame.
    last_time: Option<DateTime<Local>>,
}

impl Dashboard {
    /// Draw the backdrop (grid, scanlines, frames, labels and the hardware
    /// names from `snapshot`) and start the canvas as a copy of it. Whether
    /// the GPU panel shows figures is decided here, from `snapshot.gpu`.
    pub fn new(snapshot: &Snapshot) -> Self {
        let font = Font::default_font();
        let backdrop = draw_backdrop(snapshot, &font);
        let mut canvas = Canvas::new(WIDTH, HEIGHT);
        canvas.blit_from(&backdrop, SCREEN);
        Self {
            backdrop,
            canvas,
            font,
            down: Ring::new(),
            up: Ring::new(),
            gpu_present: snapshot.gpu.is_some(),
            last_time: None,
        }
    }

    /// Record the network rates of `s` (once per distinct sample time) and
    /// redraw every dynamic widget.
    pub fn render(&mut self, s: &Snapshot) {
        if self.last_time != Some(s.time) {
            self.down.push(s.net_down_bps as f32);
            self.up.push(s.net_up_bps as f32);
            self.last_time = Some(s.time);
        }
        let (c, bd, f) = (&mut self.canvas, &self.backdrop, &self.font);

        let date = s.time.format("%Y-%m-%d").to_string();
        text(c, bd, DATE_BOX, &date, f, LABEL_PX, DIM);
        let clock = s.time.format("%H:%M:%S").to_string();
        text(c, bd, CLOCK_BOX, &clock, f, CLOCK_PX, TEXT);

        text(c, bd, CPU_LOAD_BOX, &fmt_pct(s.cpu_load), f, BIG_PX, TEXT);
        text(c, bd, CPU_TEMP_BOX, &fmt_temp(s.cpu_temp_c), f, TEMP_PX, MAGENTA);
        let cores: Vec<f32> = s.cpu_cores.iter().map(|v| v / 100.0).collect();
        thread_bars(c, bd, CPU_THREADS, &cores);

        c.blit_from(bd, GPU_BODY);
        match &s.gpu {
            Some(g) if self.gpu_present => {
                text(c, bd, GPU_LOAD_BOX, &fmt_pct(g.load), f, BIG_PX, TEXT);
                text(c, bd, GPU_TEMP_BOX, &fmt_temp(g.temp_c), f, TEMP_PX, MAGENTA);
                let vram = format!("VRAM {} / {} GB", fmt_gb(g.vram_used), fmt_gb(g.vram_total));
                fitted(c, bd, VRAM_BOX, &vram, f, LABEL_PX, DIM);
                bar(c, bd, VRAM_BAR, ratio(g.vram_used, g.vram_total));
            }
            _ => text(c, bd, GPU_OFFLINE_BOX, "OFFLINE", f, TEMP_PX, DIM),
        }

        let mem = format!("{} / {} GB", fmt_gb(s.mem_used), fmt_gb(s.mem_total));
        fitted(c, bd, MEM_VALUE_BOX, &mem, f, VALUE_PX, TEXT);
        bar(c, bd, MEM_BAR, ratio(s.mem_used, s.mem_total));

        let disk = format!("{} / {} GB", fmt_gb(s.disk_used), fmt_gb(s.disk_total));
        fitted(c, bd, DISK_VALUE_BOX, &disk, f, VALUE_PX, TEXT);
        bar(c, bd, DISK_BAR, ratio(s.disk_used, s.disk_total));

        let dn = format!("DN {}", fmt_rate(s.net_down_bps));
        fitted(c, bd, NET_DN_BOX, &dn, f, VALUE_PX, CYAN);
        let up = format!("UP {}", fmt_rate(s.net_up_bps));
        fitted(c, bd, NET_UP_BOX, &up, f, VALUE_PX, MAGENTA);
        sparkline(c, bd, NET_DN_SPARK, &self.down, CYAN);
        sparkline(c, bd, NET_UP_SPARK, &self.up, MAGENTA);
    }

    /// Make the next flush send the full frame (after a reconnect the panel
    /// content is unknown). Delegates to [`Canvas::invalidate`].
    pub fn invalidate(&mut self) {
        self.canvas.invalidate();
    }
}

/// `used / total` clamped to `0..=1`; `0` when `total` is zero.
fn ratio(used: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        (used as f64 / total as f64).clamp(0.0, 1.0) as f32
    }
}

/// [`text_box`] over `r`.
fn text(c: &mut Canvas, bd: &Canvas, r: Rect, s: &str, f: &Font, px: f32, color: Color) {
    text_box(c, bd, r.x, r.y, r.w, r.h, s, f, px, color);
}

/// [`text_box`] over `r`, shrinking `px` until `s` fits the box width.
fn fitted(c: &mut Canvas, bd: &Canvas, r: Rect, s: &str, f: &Font, px: f32, color: Color) {
    text(c, bd, r, s, f, fit_px(s, f, px, r.w), color);
}

/// Blend a 1-px outline just inside `r`, touching each pixel once.
fn blend_outline(c: &mut Canvas, r: Rect, color: Color, alpha: u8) {
    if r.w < 2 || r.h < 2 {
        return c.blend_rect(r, color, alpha);
    }
    c.blend_rect(Rect { h: 1, ..r }, color, alpha);
    c.blend_rect(Rect { y: r.y + r.h - 1, h: 1, ..r }, color, alpha);
    c.blend_rect(Rect { y: r.y + 1, w: 1, h: r.h - 2, ..r }, color, alpha);
    c.blend_rect(Rect { x: r.x + r.w - 1, y: r.y + 1, w: 1, h: r.h - 2 }, color, alpha);
}

/// The static parts of the dashboard: background, grid, scanlines, frames,
/// dividers, title, LIVE dot, labels and hardware names.
fn draw_backdrop(s: &Snapshot, font: &Font) -> Canvas {
    let mut c = Canvas::new(WIDTH, HEIGHT);
    c.fill(BG);
    for x in (0..WIDTH).step_by(GRID_STEP as usize) {
        c.blend_rect(Rect { x, y: 0, w: 1, h: HEIGHT }, CYAN, GRID_ALPHA);
    }
    for y in (0..HEIGHT).step_by(GRID_STEP as usize) {
        c.blend_rect(Rect { x: 0, y, w: WIDTH, h: 1 }, CYAN, GRID_ALPHA);
    }
    for y in (SCANLINE_EVERY - 1..HEIGHT).step_by(SCANLINE_EVERY as usize) {
        c.blend_rect(Rect { x: 0, y, w: WIDTH, h: 1 }, Color::BLACK, SCANLINE_ALPHA);
    }
    for frame in FRAMES {
        blend_outline(&mut c, grow(frame), CYAN, FRAME_GLOW_ALPHA);
        blend_outline(&mut c, frame, CYAN, FRAME_ALPHA);
    }
    c.blend_rect(Rect { x: 0, y: DIVIDER_Y, w: WIDTH, h: 1 }, CYAN, FRAME_ALPHA);

    c.draw_text(TITLE_POS.0, TITLE_POS.1, TITLE, font, TITLE_PX, CYAN);
    c.blend_rect(grow(LIVE_DOT), MAGENTA, BAR_GLOW_ALPHA);
    c.fill_rect(LIVE_DOT, MAGENTA);

    let mut label = |(x, y): (u16, u16), s: &str| {
        c.draw_text(x, y, s, font, LABEL_PX, DIM);
    };
    label(CPU_LABEL, "CPU");
    label(CPU_NAME, &truncate_to_width(&s.cpu_name, font, LABEL_PX, NAME_MAX_W));
    label(GPU_LABEL, "GPU");
    if let Some(g) = &s.gpu {
        label(GPU_NAME, &truncate_to_width(&g.name, font, LABEL_PX, NAME_MAX_W));
    }
    label(MEM_LABEL, "MEMORY");
    label(DISK_LABEL, format!("DISK {}", s.disk_label).trim_end());
    label(NET_LABEL, "NET");
    c
}

/// Wake `display` for the dashboard: screen on, portrait (upside down when
/// `flip` is set, for a panel mounted with its cable at the other edge),
/// full brightness.
pub fn wake<W: Write>(display: &mut Display<W>, flip: bool) -> turing_device::Result<()> {
    let o = if flip { Orientation::PortraitFlipped } else { Orientation::Portrait };
    display.init(o, BRIGHTNESS)
}

/// Open the display on `port` (or the first discovered one) and [`wake`] it.
fn connect(port: Option<&str>, flip: bool, log: &mut dyn Write) -> Result<Display<impl Write>, Box<dyn Error>> {
    let port = match port {
        Some(p) => p.to_string(),
        None => discover()?
            .first()
            .ok_or("no Turing display found (use --port to force one)")?
            .port
            .clone(),
    };
    let mut display = Display::open(&port)?;
    wake(&mut display, flip)?;
    let _ = writeln!(log, "display: {port}");
    Ok(display)
}

/// Run the dashboard on the serial display at `port` (auto-discovered when
/// `None`), in portrait (flipped when `flip` is set), refreshing every
/// `interval`; stop after `ticks` rendered frames, or never. The port is
/// released when this returns.
///
/// With `ticks` set, fails if no display can be opened at start. Otherwise
/// display errors, including a display missing at start, are logged and
/// retried every [`RETRY`]; the loop never dies. Silent unless `debug` is
/// set, which logs progress and errors to stderr.
pub fn run(
    sampler: &mut dyn Sampler,
    port: Option<String>,
    flip: bool,
    interval: Duration,
    ticks: Option<u64>,
    debug: bool,
) -> Result<(), Box<dyn Error>> {
    let mut log: Box<dyn Write> = if debug { Box::new(io::stderr()) } else { Box::new(io::sink()) };
    let mut log_for_connect: Box<dyn Write> = if debug { Box::new(io::stderr()) } else { Box::new(io::sink()) };
    run_with(sampler, &mut || connect(port.as_deref(), flip, &mut log_for_connect), interval, ticks, RETRY, &mut log)
}

/// The run loop behind [`run`], with the display factory and retry delay
/// injectable for tests.
///
/// Opens the display: if that fails, the error is returned when `ticks` is
/// set (a bounded run), otherwise the loop starts without a display and
/// retries like after a reconnect. Then takes a warm-up sample and sleeps
/// one `interval` so CPU loads are valid on the first frame, and per tick:
/// sample, render, flush, and log one line to `log`. A flush error drops
/// the display; `connect` is retried no more often than every `retry`, and
/// a successful (re)connect forces a full frame. When `interval` is a whole
/// number of seconds, ticks after the first wake just after wall-clock
/// boundaries (see [`delay_to_aligned_tick`]) so the displayed clock never
/// skips or repeats a second; otherwise they are aligned to `interval`
/// boundaries of a monotonic clock. Either way a slow tick skips missed
/// boundaries.
pub fn run_with<W: Write>(
    sampler: &mut dyn Sampler,
    connect: &mut dyn FnMut() -> Result<Display<W>, Box<dyn Error>>,
    interval: Duration,
    ticks: Option<u64>,
    retry: Duration,
    log: &mut dyn Write,
) -> Result<(), Box<dyn Error>> {
    let interval = interval.max(Duration::from_millis(1));
    let mut display = match connect() {
        Ok(d) => Some(d),
        Err(e) if ticks.is_some() => return Err(e),
        Err(e) => {
            let _ = writeln!(log, "no display yet: {e}; retrying every {retry:?}");
            None
        }
    };
    let mut dash = Dashboard::new(&sampler.sample());
    thread::sleep(interval);

    let done = |tick: u64| ticks.is_some_and(|n| tick >= n);
    let mut tick = 0u64;
    let mut last_attempt = Instant::now();
    let mut next = Instant::now();
    while !done(tick) {
        if display.is_none() && last_attempt.elapsed() >= retry {
            last_attempt = Instant::now();
            match connect() {
                Ok(d) => {
                    let _ = writeln!(log, "display connected");
                    dash.invalidate();
                    display = Some(d);
                }
                Err(e) => {
                    let _ = writeln!(log, "reconnect failed: {e}");
                }
            }
        }

        let started = Instant::now();
        let snapshot = sampler.sample();
        dash.render(&snapshot);
        tick += 1;
        match display.as_mut().map(|d| dash.canvas.flush(d)) {
            Some(Ok(rects)) => {
                let bytes: usize = rects.iter().map(|r| 6 + r.byte_len()).sum();
                let ms = started.elapsed().as_millis();
                let _ = writeln!(log, "tick {tick}: {} rect(s), {bytes} bytes, {ms} ms", rects.len());
            }
            Some(Err(e)) => {
                let _ = writeln!(log, "tick {tick}: display error: {e}; retrying every {retry:?}");
                display = None;
                last_attempt = Instant::now();
            }
            None => {
                let _ = writeln!(log, "tick {tick}: display offline");
            }
        }

        if done(tick) {
            break;
        }
        let delay = delay_to_aligned_tick(wall_clock(), interval).unwrap_or_else(|| {
            next += interval;
            let now = Instant::now();
            while next <= now {
                next += interval;
            }
            next - now
        });
        thread::sleep(delay);
    }
    Ok(())
}
