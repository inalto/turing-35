# Cyberpunk telemetry dashboard — design

Date: 2026-10-27. Status: approved in chat.

## Goal

A Rust program (`turing-dashboard`, new binary crate in `turing-screen/`) that draws a neon
cyberpunk dashboard on the Turing 3.5" panel (landscape, 480×320) and refreshes live system
telemetry once per second, sending only the pixels that changed.

## Non-goals

Browser output, a generic scene language, Linux/macOS sampling (code may compile there but only
Windows is verified), CPU temperature on AMD without an external tool.

## Telemetry

| Metric | Source | Shown as |
|---|---|---|
| CPU total load, per-thread load (16 on the 5700G) | `sysinfo` | big `%`, 16 thin bars |
| CPU temperature | `sysinfo` components; `None` when absent | `61°C` or `--` |
| GPU name, load, temperature, VRAM used/total | `nvml-wrapper`; `None` when NVML fails to load | `%`, `°C`, bar |
| RAM used/total | `sysinfo` | `12.4 / 31.0 GB`, bar |
| System disk used/total | `sysinfo` disk whose mount point is the system drive (`C:\` on Windows, `/` elsewhere), else the first disk | `412 / 931 GB`, bar |
| Network down/up throughput of the busiest interface | `sysinfo` networks, delta between samples ÷ elapsed | `DN 1.8 MB/s`, `UP 212 KB/s`, two 60-s sparklines |
| Date and local time | `chrono::Local` | `2026-10-27`, `17:42:08` |

## Visual design

Canvas 480×320, landscape. Palette:

| Name | Hex | Use |
|---|---|---|
| `BG` | `#07060F` | background |
| `CYAN` | `#00F0FF` | title, frames, CPU thread bars, down sparkline |
| `MAGENTA` | `#FF2BD6` | bars, temperatures, up sparkline, LIVE dot |
| `YELLOW` | `#F9F002` | bar colour when value ≥ 85 % |
| `TEXT` | `#F2F2FF` | numbers |
| `DIM` | `#6E6A8C` | labels, date, hardware names |

Static **backdrop** drawn once at start: `BG` fill; grid lines every 24 px blended `CYAN` at
alpha 10; scanlines (every 3rd row) blended black at alpha 40; panel frames as 1-px `CYAN`
outlines at alpha 110 with a 1-px outer glow at alpha 40; all labels and hardware names.
Dynamic widgets restore their rect from the backdrop, then redraw.

### Layout (x, y, w, h)

Top bar (0,0,480,36): title `SYS://TELEMETRY` at (12,6) size 22 `CYAN`; date at (300,12)
size 14 `DIM`; clock `HH:MM:SS` at (376,4) size 26 `TEXT`; `LIVE` dot: 6×6 `MAGENTA` at
(462,15) with glow. Divider line y=38 (`CYAN` alpha 110), vertical divider x=240 from y=38 to 320.

Left column (0..240):
- CPU panel frame (6,42,228,128). Label `CPU` (12,46) size 14 `DIM`; CPU name (48,46) size 14
  `DIM`, truncated to fit 180 px. Load `NN%` at (12,62) size 48 `TEXT`. Temperature at
  (140,74) size 26 `MAGENTA` (`--` when unknown). Thread bars: `n` bars in (12,124,216,40),
  each `max(4, 216/n - 2)` wide, height = load × 40, `CYAN`, drawn bottom-up; track blended
  `CYAN` alpha 25.
- GPU panel frame (6,176,228,136). Label `GPU` (12,180); name (48,180) truncated to 180 px.
  Load at (12,196) size 48; temperature at (140,208) size 26 `MAGENTA`. `VRAM 4.9 / 6.0 GB`
  at (12,258) size 14 `DIM`; bar (12,280,216,14). When no NVML: `OFFLINE` at (12,210) size
  26 `DIM`, no bars.

Right column (248..474):
- Memory frame (246,42,228,66): label `MEMORY` (252,46); value `12.4 / 31.0 GB` at (252,62)
  size 18 `TEXT`; bar (252,88,216,14).
- Disk frame (246,112,228,66): label `DISK C:` (252,116); value at (252,132) size 18; bar
  (252,158,216,14).
- Net frame (246,182,228,130): label `NET` (252,186); `DN 1.8 MB/s` at (252,204) size 18
  `CYAN`; `UP 212 KB/s` at (362,204) size 18 `MAGENTA`; down sparkline in (252,232,216,36)
  `CYAN`; up sparkline in (252,272,216,36) `MAGENTA`.

**Bar**: track = rect blended `CYAN` alpha 30; fill = `MAGENTA` (or `YELLOW` ≥ 85 %) of
width `round(value × w)`; glow = fill rect expanded by 1 px blended in the fill colour at
alpha 60 drawn before the fill. **Sparkline**: polyline of the 60 most recent samples left
to right across the rect, y scaled to the window maximum (minimum scale 1 KB/s), 1-px
line plus a 1-px blended glow (alpha 70) one row below; baseline blended `CYAN` alpha 30.

## Architecture

```
turing-screen/crates/turing-dashboard/
  src/main.rs        CLI: [--port COMx] [--interval MS=1000] [--ticks N] (exit after N ticks; default run forever)
  src/telemetry.rs   Snapshot, GpuStats, Sampler trait, SysInfoSampler, Ring (60 samples), format helpers
  src/theme.rs       colour constants, font sizes, layout rects
  src/widgets.rs     bar, thread bars, sparkline, text widgets (each erases its rect then draws)
  src/dashboard.rs   Dashboard { backdrop, canvas, rings } :: new(w,h,&Snapshot) / render(&Snapshot) / run loop
```

`turing-canvas` gains four primitives used above: `blend_rect(rect, color, alpha)`,
`draw_line(x0, y0, x1, y1, color)` (Bresenham, clipped), `draw_rect_outline(rect, color)` and
`blit_from(&Canvas, rect)` (copy a rect from a same-sized canvas).

Loop per tick: `snapshot = sampler.sample()`; push net values into rings; `dashboard.render(&snapshot)`;
`canvas.flush(&display)`; sleep until the next interval boundary. Serial write error → log,
drop the display, re-discover every 2 s, re-init (screen on, orientation, brightness 0) and
force a full redraw (`Dashboard::invalidate()` resets the canvas baseline).

## Error handling

NVML missing or failing → `gpu: None`, panel shows `OFFLINE`. No disks/networks → zero values.
Sampler never panics. Display errors never kill the loop.

## Testing

- Canvas primitives: pixel-level unit tests (blend math, line endpoints and clipping, outline, blit_from).
- Telemetry: `Ring` push/order/max; format helpers; `SysInfoSampler` smoke test (`sample()` returns
  plausible totals, never panics).
- Dashboard with a `FakeSampler`: bar fill width proportional to value; yellow at ≥ 85 %;
  changing only CPU load produces flush rects entirely in `x < 240`; sparkline pixels stay in
  their rect; `OFFLINE` path renders; `render` is idempotent (second flush sends nothing).
- Live run on COM20 with `--ticks 10` at the end.
