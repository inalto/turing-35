//! Rendering and run-loop tests against in-memory canvases and display sinks.
use std::cell::Cell;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{Local, TimeZone};
use turing_canvas::{Canvas, Color, Font};
use turing_dashboard::dashboard::{run_with, Dashboard};
use turing_dashboard::telemetry::{GpuStats, Ring, Sampler, Snapshot};
use turing_dashboard::theme::*;
use turing_dashboard::widgets::{bar, sparkline, text_box, thread_bars};
use turing_device::Display;
use turing_proto::{bitmap_header, Rect};

const GIB: u64 = 1 << 30;

fn snapshot() -> Snapshot {
    Snapshot {
        cpu_name: "AMD Ryzen 7 5700G with Radeon Graphics".into(),
        cpu_load: 37.0,
        cpu_cores: (0..16).map(|i| (i * 6) as f32).collect(),
        cpu_temp_c: None,
        gpu: Some(GpuStats {
            name: "NVIDIA GeForce GTX 1660 SUPER".into(),
            load: 12.0,
            temp_c: Some(48.0),
            vram_used: 2 * GIB,
            vram_total: 6 * GIB,
        }),
        mem_used: 12 * GIB,
        mem_total: 31 * GIB,
        disk_label: "C:".into(),
        disk_used: 412 * GIB,
        disk_total: 931 * GIB,
        net_down_bps: 1.8 * 1024.0 * 1024.0,
        net_up_bps: 212.0 * 1024.0,
        time: Local.with_ymd_and_hms(2026, 10, 27, 17, 42, 8).unwrap(),
    }
}

/// Always returns the same snapshot.
struct FakeSampler(Snapshot);

impl Sampler for FakeSampler {
    fn sample(&mut self) -> Snapshot {
        self.0.clone()
    }
}

fn sink() -> Display<Vec<u8>> {
    Display::new(Vec::new(), None)
}

fn backdrop() -> Canvas {
    let mut c = Canvas::landscape();
    c.fill(BG);
    c
}

/// A copy of `src` (canvases are not `Clone`).
fn copy(src: &Canvas) -> Canvas {
    let mut c = Canvas::landscape();
    c.blit_from(src, SCREEN);
    c
}

/// Every pixel where `a` and `b` differ.
fn diff(a: &Canvas, b: &Canvas) -> Vec<(u16, u16)> {
    let mut out = Vec::new();
    for y in 0..a.height() {
        for x in 0..a.width() {
            if a.pixel(x, y) != b.pixel(x, y) {
                out.push((x, y));
            }
        }
    }
    out
}

fn inside(r: Rect, (x, y): (u16, u16)) -> bool {
    x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h
}

fn count(c: &Canvas, r: Rect, color: Color) -> usize {
    (r.y..r.y + r.h)
        .flat_map(|y| (r.x..r.x + r.w).map(move |x| (x, y)))
        .filter(|&(x, y)| c.pixel(x, y) == color)
        .count()
}

/// `c` blended over `under` at `alpha`, as the canvas computes it.
fn blended(under: Color, c: Color, alpha: u8) -> Color {
    let mut px = Canvas::new(1, 1);
    px.fill(under);
    px.blend_rect(Rect { x: 0, y: 0, w: 1, h: 1 }, c, alpha);
    px.pixel(0, 0)
}

#[test]
fn bar_fill_width_is_rounded_fraction_of_the_width() {
    let bd = backdrop();
    let r = MEM_BAR;
    let mid = r.y + r.h / 2;
    for (v, color) in [(0.5f32, MAGENTA), (0.9, YELLOW), (0.333, MAGENTA), (0.85, YELLOW)] {
        let mut c = copy(&bd);
        bar(&mut c, &bd, r, v);
        let fw = (v * r.w as f32).round() as u16;
        for x in r.x..r.x + fw {
            assert_eq!(c.pixel(x, mid), color, "value {v}: x {x} inside the fill");
        }
        assert_ne!(c.pixel(r.x + fw, mid), color, "value {v}: fill ends at {fw}");
        let end = r.x + r.w - 1;
        assert_eq!(c.pixel(end, mid), blended(BG, CYAN, BAR_TRACK_ALPHA), "track is drawn");
    }
}

#[test]
fn bar_redraw_shrinks_back_from_the_backdrop() {
    let bd = backdrop();
    let mut big = copy(&bd);
    bar(&mut big, &bd, MEM_BAR, 0.9);
    bar(&mut big, &bd, MEM_BAR, 0.1);
    let mut small = copy(&bd);
    bar(&mut small, &bd, MEM_BAR, 0.1);
    assert!(diff(&big, &small).is_empty());
}

#[test]
fn thread_bars_grow_bottom_up_inside_their_rect() {
    let bd = backdrop();
    let mut c = copy(&bd);
    let loads: Vec<f32> = (0..16).map(|i| i as f32 / 15.0).collect();
    thread_bars(&mut c, &bd, CPU_THREADS, &loads);
    assert!(diff(&c, &bd).iter().all(|&p| inside(CPU_THREADS, p)));
    let bw = (CPU_THREADS.w / 16 - THREAD_GAP).max(THREAD_MIN_W);
    let last_x = CPU_THREADS.x + 15 * (bw + THREAD_GAP);
    let bottom = CPU_THREADS.y + CPU_THREADS.h - 1;
    assert_eq!(c.pixel(last_x, CPU_THREADS.y), CYAN, "full bar reaches the top");
    assert_eq!(c.pixel(last_x, bottom), CYAN, "bars are drawn from the bottom");
    let track = blended(BG, CYAN, THREAD_TRACK_ALPHA);
    assert_eq!(c.pixel(CPU_THREADS.x, bottom), track, "empty bar shows only its track");
    assert_eq!(c.pixel(CPU_THREADS.x + bw, bottom), BG, "gap between bars");
}

#[test]
fn sparkline_with_60_samples_stays_inside_its_rect() {
    let bd = backdrop();
    let mut ring = Ring::new();
    for i in 0..Ring::LEN {
        ring.push(if i % 7 == 0 { 0.0 } else { (i * 1000) as f32 });
    }
    let mut c = copy(&bd);
    sparkline(&mut c, &bd, NET_DN_SPARK, &ring, CYAN);
    let changed = diff(&c, &bd);
    assert!(!changed.is_empty());
    assert!(changed.iter().all(|&p| inside(NET_DN_SPARK, p)));
    // The newest (largest) sample touches the top row at the right edge.
    let right = NET_DN_SPARK.x + NET_DN_SPARK.w - 1;
    assert_eq!(c.pixel(right, NET_DN_SPARK.y), CYAN);
}

#[test]
fn empty_sparkline_paints_only_the_baseline() {
    let bd = backdrop();
    let mut c = copy(&bd);
    sparkline(&mut c, &bd, NET_UP_SPARK, &Ring::new(), MAGENTA);
    let r = NET_UP_SPARK;
    let baseline = Rect { x: r.x, y: r.y + r.h - 1, w: r.w, h: 1 };
    let changed = diff(&c, &bd);
    assert_eq!(changed.len(), r.w as usize);
    assert!(changed.iter().all(|&p| inside(baseline, p)));
}

#[test]
fn text_box_clips_to_its_box_and_restores_old_text() {
    let font = Font::default_font();
    let bd = backdrop();
    let mut c = copy(&bd);
    let r = Rect { x: 100, y: 100, w: 40, h: 20 };
    text_box(&mut c, &bd, r.x, r.y, r.w, r.h, "WWWWWWWWWW", &font, 26.0, TEXT);
    let changed = diff(&c, &bd);
    assert!(!changed.is_empty());
    assert!(changed.iter().all(|&p| inside(r, p)));
    text_box(&mut c, &bd, r.x, r.y, r.w, r.h, "", &font, 26.0, TEXT);
    assert!(diff(&c, &bd).is_empty());
}

#[test]
fn first_flush_is_a_full_frame_and_rerendering_sends_nothing() {
    let s = snapshot();
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    let mut d = sink();
    assert_eq!(dash.canvas.flush(&mut d).unwrap(), vec![SCREEN]);
    dash.render(&s);
    assert_eq!(dash.canvas.flush(&mut d).unwrap(), Vec::<Rect>::new());
}

#[test]
fn invalidate_makes_the_next_flush_a_full_frame() {
    let s = snapshot();
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    let mut d = sink();
    dash.canvas.flush(&mut d).unwrap();
    dash.invalidate();
    assert_eq!(dash.canvas.flush(&mut d).unwrap(), vec![SCREEN]);
}

#[test]
fn cpu_only_change_flushes_only_the_left_column() {
    let mut s = snapshot();
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    let mut d = sink();
    dash.canvas.flush(&mut d).unwrap();
    s.cpu_load = 88.0;
    s.cpu_cores = vec![95.0; 16];
    dash.render(&s);
    let sent = dash.canvas.flush(&mut d).unwrap();
    assert!(!sent.is_empty());
    assert!(sent.iter().all(|r| r.x + r.w <= 240), "{sent:?}");
}

#[test]
fn busy_tick_never_flushes_a_rect_across_the_column_divider() {
    // CPU, GPU, memory, disk and network all change in the same tick, so the
    // left and right columns change on the same rows; each flushed rect must
    // still stay on one side of the divider.
    let mut s = snapshot();
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    let mut d = sink();
    dash.canvas.flush(&mut d).unwrap();
    s.time += chrono::Duration::seconds(1);
    s.cpu_load = 88.0;
    s.cpu_cores = vec![95.0; 16];
    s.cpu_temp_c = Some(71.0);
    if let Some(g) = s.gpu.as_mut() {
        g.load = 97.0;
        g.temp_c = Some(80.0);
        g.vram_used = 5 * GIB;
    }
    s.mem_used = 27 * GIB;
    s.disk_used = 900 * GIB;
    s.net_down_bps = 85.0 * 1024.0 * 1024.0;
    s.net_up_bps = 9.0 * 1024.0 * 1024.0;
    dash.render(&s);
    let sent = dash.canvas.flush(&mut d).unwrap();
    assert!(sent.iter().any(|r| r.x + r.w <= COLUMN_DIVIDER_X && r.y > DIVIDER_Y), "{sent:?}");
    assert!(sent.iter().any(|r| r.x >= COLUMN_DIVIDER_X && r.y > DIVIDER_Y), "{sent:?}");
    for r in &sent {
        assert!(r.x + r.w <= COLUMN_DIVIDER_X || r.x >= COLUMN_DIVIDER_X, "{r:?} crosses the divider in {sent:?}");
    }
}

#[test]
fn missing_gpu_shows_offline_and_no_vram_bar() {
    let mut s = snapshot();
    s.gpu = None;
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    assert!(count(&dash.canvas, GPU_OFFLINE_BOX, DIM) > 50, "OFFLINE text drawn in DIM");
    let bar_px = count(&dash.canvas, VRAM_BAR, MAGENTA) + count(&dash.canvas, VRAM_BAR, YELLOW);
    assert_eq!(bar_px, 0);
    assert_eq!(count(&dash.canvas, GPU_LOAD_BOX, TEXT), 0, "no load figure");
}

#[test]
fn gpu_going_offline_replaces_its_figures() {
    let s = snapshot();
    let mut offline = s.clone();
    offline.gpu = None;
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    assert!(count(&dash.canvas, VRAM_BAR, MAGENTA) > 0);
    assert!(count(&dash.canvas, GPU_LOAD_BOX, TEXT) > 0);
    dash.render(&offline);
    assert_eq!(count(&dash.canvas, VRAM_BAR, MAGENTA), 0);
    assert!(count(&dash.canvas, GPU_OFFLINE_BOX, DIM) > 50);
}

#[test]
fn clock_tick_flushes_a_rect_inside_the_top_bar() {
    let s = snapshot();
    let mut later = s.clone();
    later.time = s.time + chrono::Duration::seconds(1);
    let mut dash = Dashboard::new(&s);
    dash.render(&s);
    let mut d = sink();
    dash.canvas.flush(&mut d).unwrap();
    dash.render(&later);
    let sent = dash.canvas.flush(&mut d).unwrap();
    let top: Vec<&Rect> = sent.iter().filter(|r| r.y < TOP_BAR.h).collect();
    assert!(!top.is_empty(), "{sent:?}");
    let clock_right = CLOCK_BOX.x + CLOCK_BOX.w;
    for r in top {
        assert!(r.x >= CLOCK_BOX.x && r.x + r.w <= clock_right, "{sent:?}");
        assert!(r.y + r.h <= TOP_BAR.h, "{sent:?}");
    }
}

#[test]
fn widest_clock_ends_before_the_live_dot_glow() {
    let font = Font::default_font();
    let mut c = Canvas::landscape();
    let painted = c.draw_text(CLOCK_BOX.x, CLOCK_BOX.y, "00:00:00", &font, CLOCK_PX, TEXT);
    assert!(painted.x + painted.w <= CLOCK_BOX.x + CLOCK_BOX.w, "{painted:?}");
    assert!(painted.y + painted.h <= TOP_BAR.h, "{painted:?}");
}

#[test]
fn long_network_rates_shrink_to_fit_instead_of_being_clipped() {
    let long = Snapshot { net_down_bps: 1023.0 * 1024.0, net_up_bps: 1023.0 * 1024.0, ..snapshot() };
    let idle = Snapshot { net_down_bps: 0.0, net_up_bps: 0.0, ..snapshot() };
    let mut a = Dashboard::new(&long);
    a.render(&long);
    let mut b = Dashboard::new(&idle);
    b.render(&idle);
    for r in [NET_DN_BOX, NET_UP_BOX] {
        // The text never reaches the last two columns of its box...
        let edge = Rect { x: r.x + r.w - 2, y: r.y, w: 2, h: r.h };
        let a_edge = copy_rect(&a.canvas, edge);
        assert_eq!(a_edge, copy_rect(&b.canvas, edge), "{r:?}");
        // ...yet the long text does reach the right half of it.
        let right = Rect { x: r.x + r.w / 2, y: r.y, w: r.w / 2 - 2, h: r.h };
        assert_ne!(copy_rect(&a.canvas, right), copy_rect(&b.canvas, right), "{r:?}");
    }
}

fn copy_rect(c: &Canvas, r: Rect) -> Vec<Color> {
    (r.y..r.y + r.h).flat_map(|y| (r.x..r.x + r.w).map(move |x| c.pixel(x, y))).collect()
}

/// Records into a shared buffer; fails every write once `fail_after` bytes
/// have been accepted.
struct TestSink {
    fail_after: Option<usize>,
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl Write for TestSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut bytes = self.bytes.lock().unwrap();
        if self.fail_after.is_some_and(|n| bytes.len() >= n) {
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "unplugged"));
        }
        bytes.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

type Connect = Result<Display<TestSink>, Box<dyn std::error::Error>>;

/// A snapshot whose clock advances one second per sample.
struct TickingSampler(Snapshot);

impl Sampler for TickingSampler {
    fn sample(&mut self) -> Snapshot {
        self.0.time += chrono::Duration::seconds(1);
        self.0.clone()
    }
}

const FRAME_BYTES: usize = 6 + 480 * 320 * 2;

#[test]
fn run_reconnects_after_a_write_error_and_resends_a_full_frame() {
    // Connection 1 takes the first full frame, then fails on the next write;
    // connection 2 must start with a full frame even though only the clock changed.
    let sinks: Vec<Arc<Mutex<Vec<u8>>>> = vec![Default::default(), Default::default()];
    let connects = Cell::new(0);
    let mut connect = || -> Connect {
        let i = connects.get();
        connects.set(i + 1);
        let fail_after = (i == 0).then_some(FRAME_BYTES);
        Ok(Display::new(TestSink { fail_after, bytes: sinks[i].clone() }, None))
    };
    let mut sampler = TickingSampler(snapshot());
    let interval = Duration::from_millis(1);
    run_with(&mut sampler, &mut connect, interval, Some(4), Duration::ZERO).unwrap();
    assert_eq!(connects.get(), 2);
    assert_eq!(sinks[0].lock().unwrap().len(), FRAME_BYTES);
    let second = sinks[1].lock().unwrap();
    assert_eq!(&second[..6], &bitmap_header(SCREEN));
    assert!(second.len() > FRAME_BYTES, "full frame, then the next tick's clock update");
}

#[test]
fn run_survives_a_display_that_fails_immediately() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let connects = Cell::new(0);
    let mut connect = || -> Connect {
        connects.set(connects.get() + 1);
        let fail_after = (connects.get() == 1).then_some(0);
        Ok(Display::new(TestSink { fail_after, bytes: bytes.clone() }, None))
    };
    let mut sampler = FakeSampler(snapshot());
    let interval = Duration::from_millis(1);
    run_with(&mut sampler, &mut connect, interval, Some(3), Duration::ZERO).unwrap();
    assert_eq!(connects.get(), 2);
    let sent = bytes.lock().unwrap();
    assert_eq!(&sent[..6], &bitmap_header(SCREEN));
    assert_eq!(sent.len(), FRAME_BYTES, "full frame once, then nothing changed");
}

#[test]
fn bounded_run_fails_fast_when_no_display_can_be_opened() {
    let mut connect = || -> Connect { Err("no display".into()) };
    let mut sampler = FakeSampler(snapshot());
    let interval = Duration::from_millis(1);
    assert!(run_with(&mut sampler, &mut connect, interval, Some(1), Duration::ZERO).is_err());
}

/// Counts how often it is sampled.
struct CountingSampler(Snapshot, usize);

impl Sampler for CountingSampler {
    fn sample(&mut self) -> Snapshot {
        self.1 += 1;
        self.0.clone()
    }
}

#[test]
fn run_takes_one_warm_up_sample_before_the_first_frame() {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let mut connect = || -> Connect { Ok(Display::new(TestSink { fail_after: None, bytes: bytes.clone() }, None)) };
    let mut sampler = CountingSampler(snapshot(), 0);
    run_with(&mut sampler, &mut connect, Duration::from_millis(1), Some(2), Duration::ZERO).unwrap();
    assert_eq!(sampler.1, 3, "warm-up + one per tick");
}

#[test]
fn run_without_ticks_keeps_retrying_when_no_display_is_found_at_start() {
    // An always-on dashboard must survive the device enumerating late. The
    // loop never returns by itself, so the connect closure stops it with a
    // panic after a few failed attempts; returning before that is a failure.
    let attempts = Cell::new(0);
    let mut connect = || -> Connect {
        attempts.set(attempts.get() + 1);
        if attempts.get() == 4 {
            panic!("stop: enough attempts");
        }
        Err("no display".into())
    };
    let mut sampler = FakeSampler(snapshot());
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_with(&mut sampler, &mut connect, Duration::from_millis(1), None, Duration::ZERO)
    }));
    assert!(outcome.is_err(), "run returned instead of retrying: {:?}", outcome.ok());
    assert_eq!(attempts.get(), 4);
}
