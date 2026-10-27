# Cyberpunk dashboard — implementation plan

Spec: `docs/superpowers/specs/2026-10-27-cyberpunk-dashboard-design.md` (binding authority).
Workspace: `turing-screen/` (Cargo workspace, Rust 1.93). Branch: `feature/cyberpunk-dashboard`.

## Global constraints

- TDD: write the failing test, run it, then implement. `cargo test --workspace` and
  `cargo clippy --workspace --all-targets` must be clean before every commit.
- Existing crates: `turing-proto` (header packing, `Rect {x,y,w,h}` u16, `rgb565`), `turing-device`
  (`Display<W: Write>`, `Display::new(sink, pacing)`, `discover()`, `Display::open(port)`, `init`,
  `blit`, `size()`), `turing-canvas` (`Canvas::new(w,h)`, `fill`, `fill_rect`, `draw_rgb`,
  `draw_text(x,y,text,&Font,px,Color) -> Rect`, `measure_text`, `flush(&mut Display) -> Result<Vec<Rect>>`,
  `Color::rgb`, `Font::default_font()`). Read `turing-screen/crates/turing-canvas/src/lib.rs` before touching it.
- Follow the existing style: small focused files, doc comments on public items, tests in `tests/`
  for integration-level behaviour and `#[cfg(test)]` modules for private helpers.
- Commit after each task with a message ending in `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`.
- Never run the vendor app or leave a process holding COM20 after a task.

## Task 1 — canvas primitives (crate `turing-canvas`)

Add to `Canvas` in `turing-screen/crates/turing-canvas/src/lib.rs`, with tests in a new
`tests/primitives.rs`:

1. `pub fn blend_rect(&mut self, rect: Rect, c: Color, alpha: u8)` — alpha-blend `c` over every
   pixel of `rect`, clipped to the canvas. Blend formula per channel: `(c*a + d*(255-a))/255`
   (reuse the private `blend`). Tests: alpha 255 equals `fill_rect`; alpha 0 leaves pixels
   untouched; alpha 128 of white over black gives `Color::rgb(128,128,128)`; clipping at the
   edge paints only the visible part.
2. `pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: Color)` — Bresenham,
   every pixel clipped individually (coordinates may be negative or beyond the canvas).
   Tests: horizontal line paints exactly `|x1-x0|+1` pixels in that row; vertical likewise;
   diagonal (0,0)-(5,5) paints the 6 diagonal pixels and nothing else; a line from
   (-5,-5) to (3,3) paints only the 4 in-canvas pixels and does not panic; endpoints are inclusive.
3. `pub fn draw_rect_outline(&mut self, rect: Rect, c: Color)` — 1-px outline just inside
   `rect`, clipped. Tests: 10×10 outline paints 36 pixels; interior untouched; a rect partly
   off-canvas paints only visible edges.
4. `pub fn blit_from(&mut self, src: &Canvas, rect: Rect)` — copy `rect` from `src` (same size
   as `self`, panic with a clear message otherwise), clipped. Tests: after filling `src` red
   and `self` black, `blit_from(rect)` makes exactly `rect.w*rect.h` red pixels; a mismatched
   size panics.

Run `cargo test -p turing-canvas` (all three test files) and clippy. Commit:
`turing-canvas: blend_rect, draw_line, draw_rect_outline, blit_from`.

## Task 2 — telemetry (new crate `turing-dashboard`, module `telemetry`)

Scaffold `turing-screen/crates/turing-dashboard/` (add to workspace members; `[[bin]]`
`turing-dashboard` with a placeholder `main.rs` that prints "not yet" and exits 0; `src/lib.rs`
with `pub mod telemetry;`). Dependencies: `sysinfo` (latest 0.3x), `nvml-wrapper` (latest
0.1x), `chrono` (0.4, default features) added to the workspace table and used via
`workspace = true`. Check current versions with `cargo search <crate> --limit 1`.

`src/telemetry.rs`:

```rust
pub struct GpuStats { pub name: String, pub load: f32 /* 0..=100 */, pub temp_c: Option<f32>, pub vram_used: u64, pub vram_total: u64 }
pub struct Snapshot {
    pub cpu_name: String, pub cpu_load: f32 /* 0..=100 */, pub cpu_cores: Vec<f32> /* 0..=100 each */, pub cpu_temp_c: Option<f32>,
    pub gpu: Option<GpuStats>,
    pub mem_used: u64, pub mem_total: u64,
    pub disk_label: String /* "C:" or "/" */, pub disk_used: u64, pub disk_total: u64,
    pub net_down_bps: f64, pub net_up_bps: f64,
    pub time: chrono::DateTime<chrono::Local>,
}
pub trait Sampler { fn sample(&mut self) -> Snapshot; }
pub struct SysInfoSampler { /* sysinfo System, Disks, Networks, Option<nvml_wrapper::Nvml>, last net totals + Instant */ }
impl SysInfoSampler { pub fn new() -> Self; }   // first refresh inside; NVML init failure -> None (never panic)
impl Sampler for SysInfoSampler { ... }         // refresh_cpu_usage/memory/disks/networks; throughput = delta bytes / elapsed secs
pub struct Ring { /* 60 f32 samples, oldest first iteration */ }
impl Ring { pub const LEN: usize = 60; pub fn new() -> Self; pub fn push(&mut self, v: f32); pub fn len(&self) -> usize; pub fn is_empty(&self) -> bool;
            pub fn iter(&self) -> impl Iterator<Item = f32> + '_ /* oldest -> newest */; pub fn max(&self) -> f32 /* 0.0 when empty */ }
pub fn fmt_rate(bytes_per_s: f64) -> String   // "0 B/s", "212 KB/s", "1.8 MB/s", "1.2 GB/s"  (1 decimal for MB/GB, none for B/KB; KB = 1000? NO: use 1024-based units)
pub fn fmt_gb(bytes: u64) -> String           // "12.4" (GiB, 1 decimal)
pub fn fmt_pct(v: f32) -> String              // "37%" (rounded, clamped 0..=100)
pub fn fmt_temp(t: Option<f32>) -> String     // "61°C" or "--"
```

Tests (`tests/telemetry.rs`): `Ring` keeps the last 60 and iterates oldest first; `max` of
empty is 0.0; `fmt_rate(0.0) == "0 B/s"`, `fmt_rate(212.0*1024.0) == "212 KB/s"`,
`fmt_rate(1.8*1024.0*1024.0) == "1.8 MB/s"`; `fmt_gb(31 GiB) == "31.0"`; `fmt_pct(36.6) == "37%"`,
`fmt_pct(250.0) == "100%"`; `fmt_temp(None) == "--"`, `fmt_temp(Some(61.4)) == "61°C"`;
`SysInfoSampler::new().sample()` returns `mem_total > 0`, `cpu_cores.len() > 0`, every load in
`0..=100`, and a second `sample()` after 300 ms also succeeds (throughput ≥ 0).

Commit: `turing-dashboard: telemetry sampler, ring buffer, formatters`.

## Task 3 — dashboard rendering and loop (crate `turing-dashboard`)

Implement `theme.rs`, `widgets.rs`, `dashboard.rs`, `main.rs` exactly to the spec's "Visual
design", "Layout" and "Architecture" sections (copy the palette and every rect from the spec).

- `theme.rs`: `pub const BG/CYAN/MAGENTA/YELLOW/TEXT/DIM: Color`, layout rects as `pub const`.
- `widgets.rs`: free functions taking `&mut Canvas, &Canvas /*backdrop*/`: `bar(canvas, backdrop, rect, value01)`,
  `thread_bars(canvas, backdrop, rect, &[f32])`, `sparkline(canvas, backdrop, rect, &Ring, color)`,
  `text_box(canvas, backdrop, x, y, w, h, text, &Font, px, color)` (restore box then draw text clipped to box).
- `dashboard.rs`: `pub struct Dashboard { backdrop: Canvas, pub canvas: Canvas, font: Font, down: Ring, up: Ring, gpu_present: bool }`;
  `pub fn new(snapshot: &Snapshot) -> Self` (draws backdrop from static parts + hardware names);
  `pub fn render(&mut self, s: &Snapshot)` (pushes net rings, redraws every dynamic widget);
  `pub fn invalidate(&mut self)` (forces next flush to send a full frame — implement by
  recreating the canvas from the backdrop and re-rendering last snapshot, or add a
  `Canvas::invalidate()` that clears `last_sent` — your choice, document it).
  `pub fn run(sampler: &mut dyn Sampler, port: Option<String>, interval: Duration, ticks: Option<u64>) -> Result<(), Box<dyn Error>>`
  with the reconnect behaviour from the spec.
- `main.rs`: parse `--port`, `--interval`, `--ticks`, build `SysInfoSampler`, call `run`.
  Print one line per tick to stderr: tick number, rects sent, bytes, elapsed.

Tests (`tests/dashboard.rs`) with a `FakeSampler` producing a fixed `Snapshot`:
- `bar` fill width equals `round(value × w)` and uses `YELLOW` at 0.9, `MAGENTA` at 0.5.
- After `Dashboard::new` + `render`, flushing to `Display::new(Vec::new(), None)` sends the full frame;
  rendering the same snapshot again flushes nothing.
- Changing only `cpu_load` (and cores) then flushing yields rects with `x + w <= 240`.
- Sparkline with 60 samples paints only inside its rect; with 0 samples paints nothing but the baseline.
- `gpu: None` renders without panic and `OFFLINE` pixels appear in the GPU panel.
- Clock text at (376,4) changes between two snapshots one second apart → a flush rect inside the top bar.

Then run live: `cargo run -p turing-dashboard -- --ticks 10` (device on COM20, nothing else
holding it) and paste the per-tick lines into the report. Commit:
`turing-dashboard: cyberpunk neon dashboard with live telemetry`.
