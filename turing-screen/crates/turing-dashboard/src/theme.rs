//! Palette, font sizes and layout of the cyberpunk dashboard (320×480,
//! portrait). Panels stack in one full-width column, memory and disk side by
//! side; text boxes start at the text position and extend to the next element.

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
pub const WIDTH: u16 = 320;
/// Canvas height in pixels.
pub const HEIGHT: u16 = 480;

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
/// Gap between thread bars (narrowed to 1 px, then 0 px, when there are too
/// many threads for bars of [`THREAD_MIN_W`]).
pub const THREAD_GAP: u16 = 2;
/// Minimum thread bar width at the full [`THREAD_GAP`].
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
/// Clock size (`HH:MM:SS` is 103 px wide at this size).
pub const CLOCK_PX: f32 = 21.0;
/// CPU and GPU load.
pub const BIG_PX: f32 = 48.0;
/// Temperatures and `OFFLINE`.
pub const TEMP_PX: f32 = 26.0;
/// Memory, disk and network values.
pub const VALUE_PX: f32 = 18.0;
/// Hardware names are truncated to this width.
pub const NAME_MAX_W: u16 = 250;

/// The whole canvas.
pub const SCREEN: Rect = Rect { x: 0, y: 0, w: WIDTH, h: HEIGHT };
/// Header: title and LIVE dot on the first row, date and clock on the second.
pub const TOP_BAR: Rect = Rect { x: 0, y: 0, w: WIDTH, h: 54 };
/// Title text position.
pub const TITLE_POS: (u16, u16) = (12, 6);
/// Title text.
pub const TITLE: &str = "SYS://TELEMETRY";
/// LIVE dot, right of the title.
pub const LIVE_DOT: Rect = Rect { x: 304, y: 15, w: 6, h: 6 };
/// Date text box, second header row, left.
pub const DATE_BOX: Rect = Rect { x: 12, y: 32, w: 120, h: 17 };
/// Clock text box, second header row, right-hand side.
pub const CLOCK_BOX: Rect = Rect { x: 204, y: 27, w: 104, h: 26 };
const _: () = assert!(CLOCK_BOX.y + CLOCK_BOX.h <= TOP_BAR.h, "clock box must stay inside the header");
/// Horizontal divider under the header.
pub const DIVIDER_Y: u16 = 54;

/// Left edge of full-width panel frames.
const FRAME_X: u16 = 6;
/// Width of full-width panel frames.
const FRAME_W: u16 = 308;
/// Left edge of content inside a full-width panel.
const CONTENT_X: u16 = 12;
/// Width of content inside a full-width panel.
const CONTENT_W: u16 = 296;

/// CPU panel frame.
pub const CPU_FRAME: Rect = Rect { x: FRAME_X, y: 58, w: FRAME_W, h: 120 };
/// `CPU` label position.
pub const CPU_LABEL: (u16, u16) = (CONTENT_X, 62);
/// CPU name position.
pub const CPU_NAME: (u16, u16) = (48, 62);
/// CPU load (`NN%`) text box.
pub const CPU_LOAD_BOX: Rect = Rect { x: CONTENT_X, y: 78, w: 126, h: 57 };
/// CPU temperature text box.
pub const CPU_TEMP_BOX: Rect = Rect { x: 140, y: 90, w: 92, h: 31 };
/// Per-thread load bars.
pub const CPU_THREADS: Rect = Rect { x: CONTENT_X, y: 140, w: CONTENT_W, h: 32 };

/// GPU panel frame.
pub const GPU_FRAME: Rect = Rect { x: FRAME_X, y: 184, w: FRAME_W, h: 128 };
/// `GPU` label position.
pub const GPU_LABEL: (u16, u16) = (CONTENT_X, 188);
/// GPU name position.
pub const GPU_NAME: (u16, u16) = (48, 188);
/// Everything dynamic in the GPU panel, restored before each redraw.
pub const GPU_BODY: Rect = Rect { x: FRAME_X + 1, y: 202, w: FRAME_W - 2, h: 109 };
/// GPU load text box.
pub const GPU_LOAD_BOX: Rect = Rect { x: CONTENT_X, y: 204, w: 126, h: 57 };
/// GPU temperature text box.
pub const GPU_TEMP_BOX: Rect = Rect { x: 140, y: 216, w: 92, h: 31 };
/// `VRAM used / total GB` text box.
pub const VRAM_BOX: Rect = Rect { x: CONTENT_X, y: 264, w: CONTENT_W, h: 17 };
/// VRAM bar.
pub const VRAM_BAR: Rect = Rect { x: CONTENT_X, y: 284, w: CONTENT_W, h: 14 };
/// `OFFLINE` text box, shown when NVML is unavailable.
pub const GPU_OFFLINE_BOX: Rect = Rect { x: CONTENT_X, y: 218, w: CONTENT_W, h: 31 };

/// Width of the half-width memory and disk frames.
const HALF_FRAME_W: u16 = 150;
/// Width of content inside a half-width panel.
const HALF_CONTENT_W: u16 = 138;
/// Left edge of the disk frame (right half).
const RIGHT_FRAME_X: u16 = 164;
/// Left edge of content in the right half.
const RIGHT_CONTENT_X: u16 = 170;

/// Memory panel frame (left half).
pub const MEM_FRAME: Rect = Rect { x: FRAME_X, y: 318, w: HALF_FRAME_W, h: 66 };
/// `MEMORY` label position.
pub const MEM_LABEL: (u16, u16) = (CONTENT_X, 322);
/// Memory value text box.
pub const MEM_VALUE_BOX: Rect = Rect { x: CONTENT_X, y: 338, w: HALF_CONTENT_W, h: 22 };
/// Memory bar.
pub const MEM_BAR: Rect = Rect { x: CONTENT_X, y: 364, w: HALF_CONTENT_W, h: 14 };

/// Disk panel frame (right half).
pub const DISK_FRAME: Rect = Rect { x: RIGHT_FRAME_X, y: 318, w: HALF_FRAME_W, h: 66 };
/// `DISK C:` label position.
pub const DISK_LABEL: (u16, u16) = (RIGHT_CONTENT_X, 322);
/// Disk value text box.
pub const DISK_VALUE_BOX: Rect = Rect { x: RIGHT_CONTENT_X, y: 338, w: HALF_CONTENT_W, h: 22 };
/// Disk bar.
pub const DISK_BAR: Rect = Rect { x: RIGHT_CONTENT_X, y: 364, w: HALF_CONTENT_W, h: 14 };

/// Network panel frame.
pub const NET_FRAME: Rect = Rect { x: FRAME_X, y: 390, w: FRAME_W, h: 86 };
/// `NET` label position.
pub const NET_LABEL: (u16, u16) = (CONTENT_X, 394);
/// `DN …` text box, left half.
pub const NET_DN_BOX: Rect = Rect { x: CONTENT_X, y: 410, w: HALF_CONTENT_W, h: 22 };
/// `UP …` text box, right half.
pub const NET_UP_BOX: Rect = Rect { x: RIGHT_CONTENT_X, y: 410, w: HALF_CONTENT_W, h: 22 };
/// Download sparkline, left half.
pub const NET_DN_SPARK: Rect = Rect { x: CONTENT_X, y: 438, w: HALF_CONTENT_W, h: 32 };
/// Upload sparkline, right half.
pub const NET_UP_SPARK: Rect = Rect { x: RIGHT_CONTENT_X, y: 438, w: HALF_CONTENT_W, h: 32 };

// `Canvas::flush` sends the two halves of a row (memory | disk, DN | UP) as
// separate rects only while more than `DIRTY_GAP_PX` always-unchanged
// columns separate them: left content (and its bar glow) ends at x = 150,
// the right half's bar glow starts at x = 169 (18 columns between).
const _: () = assert!(
    RIGHT_CONTENT_X - 1 - (CONTENT_X + HALF_CONTENT_W + 1) > turing_canvas::DIRTY_GAP_PX,
    "dirty-rect split threshold must stay below the memory/disk gap"
);

/// All panel frames, drawn on the backdrop.
pub const FRAMES: [Rect; 5] = [CPU_FRAME, GPU_FRAME, MEM_FRAME, DISK_FRAME, NET_FRAME];
