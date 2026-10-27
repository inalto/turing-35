//! Dynamic dashboard widgets. Each one first restores its rect from the
//! static backdrop, then draws, so redrawing with the same input is a no-op
//! for the flushed image and a shrinking value leaves no trace.

use turing_canvas::{line_points, measure_text, Canvas, Color, Font};
use turing_proto::Rect;

use crate::telemetry::Ring;
use crate::theme::{
    BAR_GLOW_ALPHA, BAR_TRACK_ALPHA, BAR_WARN, CYAN, MAGENTA, SPARK_BASELINE_ALPHA, SPARK_GLOW_ALPHA,
    SPARK_MIN_SCALE, THREAD_GAP, THREAD_MIN_W, THREAD_TRACK_ALPHA, YELLOW,
};

/// Smallest size [`fit_px`] shrinks text to.
const MIN_FIT_PX: f32 = 8.0;

/// `rect` clipped to `canvas`, or `None` when nothing is left.
fn clip(canvas: &Canvas, rect: Rect) -> Option<Rect> {
    if rect.x >= canvas.width() || rect.y >= canvas.height() {
        return None;
    }
    let w = rect.w.min(canvas.width() - rect.x);
    let h = rect.h.min(canvas.height() - rect.y);
    (w > 0 && h > 0).then_some(Rect { x: rect.x, y: rect.y, w, h })
}

/// `rect` grown by one pixel on every side (saturating at the canvas origin).
pub(crate) fn grow(rect: Rect) -> Rect {
    let x = rect.x.saturating_sub(1);
    let y = rect.y.saturating_sub(1);
    Rect { x, y, w: rect.x + rect.w + 1 - x, h: rect.y + rect.h + 1 - y }
}

/// A finite value clamped to `0..=1` (NaN counts as 0).
fn unit(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Horizontal bar for `value01` in `0..=1`: a `CYAN` track over `rect`, then
/// a fill `round(value × w)` wide in `MAGENTA` (`YELLOW` at or above
/// [`BAR_WARN`]) with a 1-px glow around it.
pub fn bar(canvas: &mut Canvas, backdrop: &Canvas, rect: Rect, value01: f32) {
    canvas.blit_from(backdrop, grow(rect));
    canvas.blend_rect(rect, CYAN, BAR_TRACK_ALPHA);
    let v = unit(value01);
    let fw = ((v * rect.w as f32).round() as u16).min(rect.w);
    if fw == 0 {
        return;
    }
    let color = if v >= BAR_WARN { YELLOW } else { MAGENTA };
    let fill = Rect { w: fw, ..rect };
    canvas.blend_rect(grow(fill), color, BAR_GLOW_ALPHA);
    canvas.fill_rect(fill, color);
}

/// One vertical `CYAN` bar per entry of `loads01` (each `0..=1`), left to
/// right, `max(4, w / n − 2)` wide with a 2-px gap, filled bottom-up over a
/// faint track. Bars that would not fit in `rect` are not drawn.
pub fn thread_bars(canvas: &mut Canvas, backdrop: &Canvas, rect: Rect, loads01: &[f32]) {
    canvas.blit_from(backdrop, rect);
    let n = loads01.len() as u16;
    if n == 0 {
        return;
    }
    let bw = (rect.w / n).saturating_sub(THREAD_GAP).max(THREAD_MIN_W);
    for (i, &load) in loads01.iter().enumerate() {
        let x = rect.x as u32 + i as u32 * (bw + THREAD_GAP) as u32;
        if x + bw as u32 > (rect.x + rect.w) as u32 {
            break;
        }
        let x = x as u16;
        canvas.blend_rect(Rect { x, y: rect.y, w: bw, h: rect.h }, CYAN, THREAD_TRACK_ALPHA);
        let fh = ((unit(load) * rect.h as f32).round() as u16).min(rect.h);
        if fh > 0 {
            canvas.fill_rect(Rect { x, y: rect.y + rect.h - fh, w: bw, h: fh }, CYAN);
        }
    }
}

/// History graph of `ring` in `rect`: the newest sample sits at the right
/// edge, each of the [`Ring::LEN`] slots is spread evenly across the width,
/// and values are scaled to the window maximum (at least
/// [`SPARK_MIN_SCALE`]). Draws a `CYAN` baseline on the bottom row, a glow
/// one row below the line, then the 1-px line in `color`.
pub fn sparkline(canvas: &mut Canvas, backdrop: &Canvas, rect: Rect, ring: &Ring, color: Color) {
    canvas.blit_from(backdrop, rect);
    if rect.w == 0 || rect.h < 2 {
        return;
    }
    let bottom = rect.y + rect.h - 1;
    canvas.blend_rect(Rect { x: rect.x, y: bottom, w: rect.w, h: 1 }, CYAN, SPARK_BASELINE_ALPHA);

    let span = (rect.h - 2) as f32; // the line uses rows y..=bottom-1, the glow one row lower
    let scale = ring.max().max(SPARK_MIN_SCALE);
    let first_slot = Ring::LEN - ring.len();
    let points: Vec<(i32, i32)> = ring
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let slot = (first_slot + k) as f32;
            let x = rect.x as f32 + (slot * (rect.w - 1) as f32 / (Ring::LEN - 1) as f32).round();
            let v = if v.is_finite() { v.max(0.0) } else { 0.0 };
            let y = (bottom - 1) as f32 - (v / scale * span).round();
            (x as i32, y as i32)
        })
        .collect();
    if points.is_empty() {
        return;
    }

    let segments: Vec<((i32, i32), (i32, i32))> = if points.len() == 1 {
        vec![(points[0], points[0])]
    } else {
        points.windows(2).map(|w| (w[0], w[1])).collect()
    };
    for (i, &((x0, y0), (x1, y1))) in segments.iter().enumerate() {
        // Skip each joint after the first so no glow pixel is blended twice.
        for (x, y) in line_points(x0, y0, x1, y1).skip(usize::from(i > 0)) {
            let glow = Rect { x: x as u16, y: (y + 1) as u16, w: 1, h: 1 };
            canvas.blend_rect(glow, color, SPARK_GLOW_ALPHA);
        }
    }
    for &((x0, y0), (x1, y1)) in &segments {
        canvas.draw_line(x0, y0, x1, y1, color);
    }
}

/// Restore the box `(x, y, w, h)` from `backdrop`, then draw `text` with its
/// top-left at `(x, y)`; nothing is painted outside the box.
#[allow(clippy::too_many_arguments)]
pub fn text_box(
    canvas: &mut Canvas,
    backdrop: &Canvas,
    x: u16,
    y: u16,
    w: u16,
    h: u16,
    text: &str,
    font: &Font,
    px: f32,
    color: Color,
) {
    let Some(r) = clip(canvas, Rect { x, y, w, h }) else { return };
    let stride = backdrop.width() as usize * 3;
    let mut tile_rgb = Vec::with_capacity(r.w as usize * r.h as usize * 3);
    for row in r.y..r.y + r.h {
        let start = row as usize * stride + r.x as usize * 3;
        tile_rgb.extend_from_slice(&backdrop.as_rgb()[start..start + r.w as usize * 3]);
    }
    let mut tile = Canvas::new(r.w, r.h);
    tile.draw_rgb(0, 0, r.w, r.h, &tile_rgb);
    tile.draw_text(0, 0, text, font, px, color);
    canvas.draw_rgb(r.x, r.y, r.w, r.h, tile.as_rgb());
}

/// The largest size, from `px` down in 1-px steps (not below 8), at which
/// `text` is at most `max_w` pixels wide.
pub fn fit_px(text: &str, font: &Font, px: f32, max_w: u16) -> f32 {
    let mut p = px;
    while p > MIN_FIT_PX && measure_text(text, font, p).0 > max_w {
        p -= 1.0;
    }
    p
}

/// `text` shortened with a trailing `…` until it is at most `max_w` pixels
/// wide at `px`; unchanged when it already fits.
pub fn truncate_to_width(text: &str, font: &Font, px: f32, max_w: u16) -> String {
    if measure_text(text, font, px).0 <= max_w {
        return text.to_string();
    }
    let mut chars: Vec<char> = text.chars().collect();
    while !chars.is_empty() {
        chars.pop();
        let candidate = format!("{}\u{2026}", chars.iter().collect::<String>().trim_end());
        if measure_text(&candidate, font, px).0 <= max_w {
            return candidate;
        }
    }
    String::new()
}
