//! Palette, font sizes and layout of the cyberpunk dashboard (480×320,
//! landscape). Every value comes from the design spec; text boxes start at
//! the spec's text position and extend to the next element.

use turing_canvas::Color;
use turing_proto::Rect;

/// Background, `#07060F`.
pub const BG: Color = Color::rgb(0x07, 0x06, 0x0F);
/// Title, frames, CPU thread bars, down sparkline, `#00F0FF`.
pub const CYAN: Color = Color::rgb(0x00, 0xF0, 0xFF);
/// Bars, temperatures, up sparkline, LIVE dot, `#FF2BD6`.
pub const MAGENTA: Color = Color::rgb(0xFF, 0x2B, 0xD6);
/// Bar colour at or above [`BAR_WARN`], `#F9F002`.
pub const YELLOW: Color = Color::rgb(0xF9, 0xF0, 0x02);
/// Numbers, `#F2F2FF`.
pub const TEXT: Color = Color::rgb(0xF2, 0xF2, 0xFF);
/// Labels, date, hardware names, `#6E6A8C`.
pub const DIM: Color = Color::rgb(0x6E, 0x6A, 0x8C);

/// Canvas width in pixels.
pub const WIDTH: u16 = 480;
/// Canvas height in pixels.
pub const HEIGHT: u16 = 320;

/// Spacing of the backdrop grid.
pub const GRID_STEP: u16 = 24;
/// Opacity of the `CYAN` grid lines.
pub const GRID_ALPHA: u8 = 10;
/// Every this many rows a scanline is darkened.
pub const SCANLINE_EVERY: u16 = 3;
/// Opacity of the black scanlines.
pub const SCANLINE_ALPHA: u8 = 40;
/// Opacity of panel outlines and dividers.
pub const FRAME_ALPHA: u8 = 110;
/// Opacity of the 1-px outer glow around panel outlines.
pub const FRAME_GLOW_ALPHA: u8 = 40;
/// Opacity of a bar's `CYAN` track.
pub const BAR_TRACK_ALPHA: u8 = 30;
/// Opacity of the glow drawn around a bar's fill (and the LIVE dot).
pub const BAR_GLOW_ALPHA: u8 = 60;
/// Fraction at or above which a bar turns [`YELLOW`].
pub const BAR_WARN: f32 = 0.85;
/// Opacity of a thread bar's `CYAN` track.
pub const THREAD_TRACK_ALPHA: u8 = 25;
/// Gap between thread bars.
pub const THREAD_GAP: u16 = 2;
/// Minimum thread bar width.
pub const THREAD_MIN_W: u16 = 4;
/// Opacity of a sparkline's glow, one row below the line.
pub const SPARK_GLOW_ALPHA: u8 = 70;
/// Opacity of a sparkline's `CYAN` baseline.
pub const SPARK_BASELINE_ALPHA: u8 = 30;
/// Smallest full-scale value of a sparkline (1 KB/s).
pub const SPARK_MIN_SCALE: f32 = 1024.0;

/// Title size.
pub const TITLE_PX: f32 = 22.0;
/// Labels, date, hardware names and the VRAM line.
pub const LABEL_PX: f32 = 14.0;
/// Clock size. The spec asks for 26, but at 26 px Roboto's `HH:MM:SS` is
/// 103 px wide and would run over the LIVE dot; this is the largest size
/// that ends left of the dot's glow.
pub const CLOCK_PX: f32 = 21.0;
/// CPU and GPU load.
pub const BIG_PX: f32 = 48.0;
/// Temperatures and `OFFLINE`.
pub const TEMP_PX: f32 = 26.0;
/// Memory, disk and network values.
pub const VALUE_PX: f32 = 18.0;
/// Hardware names are truncated to this width.
pub const NAME_MAX_W: u16 = 180;

/// The whole canvas.
pub const SCREEN: Rect = Rect { x: 0, y: 0, w: WIDTH, h: HEIGHT };
/// Top bar holding title, date, clock and LIVE dot.
pub const TOP_BAR: Rect = Rect { x: 0, y: 0, w: 480, h: 36 };
/// Title text position.
pub const TITLE_POS: (u16, u16) = (12, 6);
/// Title text.
pub const TITLE: &str = "SYS://TELEMETRY";
/// Date text box (ends before the clock).
pub const DATE_BOX: Rect = Rect { x: 300, y: 12, w: 74, h: 17 };
/// Clock text box (ends before the LIVE dot's glow at x = 461).
pub const CLOCK_BOX: Rect = Rect { x: 376, y: 4, w: 84, h: 31 };
/// LIVE dot.
pub const LIVE_DOT: Rect = Rect { x: 462, y: 15, w: 6, h: 6 };
const _: () = assert!(CLOCK_BOX.x + CLOCK_BOX.w < LIVE_DOT.x - 1, "clock box must end before the dot's glow");
/// Horizontal divider under the top bar.
pub const DIVIDER_Y: u16 = 38;
/// Vertical divider between the columns, from [`DIVIDER_Y`] to the bottom.
pub const COLUMN_DIVIDER_X: u16 = 240;

/// CPU panel frame.
pub const CPU_FRAME: Rect = Rect { x: 6, y: 42, w: 228, h: 128 };
/// `CPU` label position.
pub const CPU_LABEL: (u16, u16) = (12, 46);
/// CPU name position.
pub const CPU_NAME: (u16, u16) = (48, 46);
/// CPU load (`NN%`) text box.
pub const CPU_LOAD_BOX: Rect = Rect { x: 12, y: 62, w: 126, h: 57 };
/// CPU temperature text box.
pub const CPU_TEMP_BOX: Rect = Rect { x: 140, y: 74, w: 92, h: 31 };
/// Per-thread load bars.
pub const CPU_THREADS: Rect = Rect { x: 12, y: 124, w: 216, h: 40 };

/// GPU panel frame.
pub const GPU_FRAME: Rect = Rect { x: 6, y: 176, w: 228, h: 136 };
/// `GPU` label position.
pub const GPU_LABEL: (u16, u16) = (12, 180);
/// GPU name position.
pub const GPU_NAME: (u16, u16) = (48, 180);
/// Everything dynamic in the GPU panel, restored before each redraw.
pub const GPU_BODY: Rect = Rect { x: 7, y: 194, w: 226, h: 117 };
/// GPU load text box.
pub const GPU_LOAD_BOX: Rect = Rect { x: 12, y: 196, w: 126, h: 57 };
/// GPU temperature text box.
pub const GPU_TEMP_BOX: Rect = Rect { x: 140, y: 208, w: 92, h: 31 };
/// `VRAM used / total GB` text box.
pub const VRAM_BOX: Rect = Rect { x: 12, y: 258, w: 216, h: 17 };
/// VRAM bar.
pub const VRAM_BAR: Rect = Rect { x: 12, y: 280, w: 216, h: 14 };
/// `OFFLINE` text box, shown when NVML is unavailable.
pub const GPU_OFFLINE_BOX: Rect = Rect { x: 12, y: 210, w: 216, h: 31 };

/// Memory panel frame.
pub const MEM_FRAME: Rect = Rect { x: 246, y: 42, w: 228, h: 66 };
/// `MEMORY` label position.
pub const MEM_LABEL: (u16, u16) = (252, 46);
/// Memory value text box.
pub const MEM_VALUE_BOX: Rect = Rect { x: 252, y: 62, w: 216, h: 22 };
/// Memory bar.
pub const MEM_BAR: Rect = Rect { x: 252, y: 88, w: 216, h: 14 };

/// Disk panel frame.
pub const DISK_FRAME: Rect = Rect { x: 246, y: 112, w: 228, h: 66 };
/// `DISK C:` label position.
pub const DISK_LABEL: (u16, u16) = (252, 116);
/// Disk value text box.
pub const DISK_VALUE_BOX: Rect = Rect { x: 252, y: 132, w: 216, h: 22 };
/// Disk bar.
pub const DISK_BAR: Rect = Rect { x: 252, y: 158, w: 216, h: 14 };

/// Network panel frame.
pub const NET_FRAME: Rect = Rect { x: 246, y: 182, w: 228, h: 130 };
/// `NET` label position.
pub const NET_LABEL: (u16, u16) = (252, 186);
/// `DN …` text box (ends before the `UP` text).
pub const NET_DN_BOX: Rect = Rect { x: 252, y: 204, w: 108, h: 22 };
/// `UP …` text box (ends inside the panel frame).
pub const NET_UP_BOX: Rect = Rect { x: 362, y: 204, w: 110, h: 22 };
/// Download sparkline.
pub const NET_DN_SPARK: Rect = Rect { x: 252, y: 232, w: 216, h: 36 };
/// Upload sparkline.
pub const NET_UP_SPARK: Rect = Rect { x: 252, y: 272, w: 216, h: 36 };

// `Canvas::flush` sends the two columns as separate rects only while more
// than `DIRTY_GAP_PX` always-unchanged columns separate them: the left
// column's widest dynamic box ends at x = 231, the right column's bar glow
// starts at x = 251 (19 columns between).
const _: () = assert!(
    MEM_BAR.x - 1 - (CPU_TEMP_BOX.x + CPU_TEMP_BOX.w) > turing_canvas::DIRTY_GAP_PX,
    "dirty-rect split threshold must stay below the column gap"
);

/// All panel frames, drawn on the backdrop.
pub const FRAMES: [Rect; 5] = [CPU_FRAME, GPU_FRAME, MEM_FRAME, DISK_FRAME, NET_FRAME];
