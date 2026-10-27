//! HTTP API daemon for the Turing 3.5" USB display.
//!
//! One render thread owns the [`Display`] and a [`Canvas`]; HTTP handlers send
//! it operations over a channel and await the reply. See `docs/RUST_REBUILD.md` §5.

use std::io::Write;
use std::sync::mpsc;
use std::thread;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use turing_canvas::{measure_text, Canvas, Color, Font};
use turing_device::Display;
use turing_proto::{Orientation, Rect};

/// Identity of the port the renderer is driving, reported by `GET /v1/device`.
#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub port: String,
    pub serial: Option<String>,
}

/// How an uploaded image is mapped onto the canvas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Scale to fit inside the panel, centred, background around it.
    #[default]
    Contain,
    /// Scale to cover the panel, centre-cropped.
    Cover,
    /// Resize to the panel size ignoring aspect ratio.
    Stretch,
    /// Draw at `(x, y)` unscaled, clipped.
    None,
}

enum Op {
    Clear(Color),
    Image { img: image::RgbImage, x: u16, y: u16, fit: Fit, bg: Color },
    Text { x: u16, y: u16, text: String, size: f32, color: Color, bg: Option<(Color, Option<Rect>)> },
    Brightness(u8),
    Power(bool),
    Orientation(Orientation),
    Status,
}

enum Out {
    Sent(Vec<Rect>),
    Text { rect: Rect, sent: Vec<Rect> },
    Status(Value),
    Done,
}

type Reply = oneshot::Sender<Result<Out, String>>;

/// Cloneable handle to the render thread.
#[derive(Clone)]
pub struct RenderHandle {
    tx: mpsc::Sender<(Op, Reply)>,
}

impl RenderHandle {
    async fn call(&self, op: Op) -> Result<Out, ApiError> {
        let (tx, rx) = oneshot::channel();
        self.tx.send((op, tx)).map_err(|_| ApiError::Device("render thread stopped".into()))?;
        rx.await.map_err(|_| ApiError::Device("render thread stopped".into()))?.map_err(ApiError::Device)
    }
}

struct Renderer<W: Write> {
    display: Display<W>,
    canvas: Canvas,
    font: Font,
    info: DeviceInfo,
    orientation: Orientation,
    brightness: u8,
    power: bool,
    connected: bool,
}

impl<W: Write> Renderer<W> {
    fn status(&self) -> Value {
        let (w, h) = self.display.size();
        json!({
            "port": self.info.port,
            "serial": self.info.serial,
            "connected": self.connected,
            "orientation": orientation_name(self.orientation),
            "brightness": self.brightness,
            "power": self.power,
            "size": { "w": w, "h": h },
        })
    }

    fn flush(&mut self) -> Result<Vec<Rect>, String> {
        self.canvas.flush(&mut self.display).map_err(|e| e.to_string())
    }

    fn handle(&mut self, op: Op) -> Result<Out, String> {
        let r = self.apply(op);
        self.connected = r.is_ok();
        r
    }

    fn apply(&mut self, op: Op) -> Result<Out, String> {
        match op {
            Op::Status => Ok(Out::Status(self.status())),
            Op::Clear(c) => {
                self.canvas.fill(c);
                self.flush().map(Out::Sent)
            }
            Op::Image { img, x, y, fit, bg } => {
                self.paint_image(img, x, y, fit, bg);
                self.flush().map(Out::Sent)
            }
            Op::Text { x, y, text, size, color, bg } => {
                if let Some((bg, rect)) = bg {
                    let rect = rect.unwrap_or_else(|| {
                        let (w, h) = measure_text(&text, &self.font, size);
                        Rect { x, y, w, h }
                    });
                    self.canvas.fill_rect(rect, bg);
                }
                let rect = self.canvas.draw_text(x, y, &text, &self.font, size, color);
                self.flush().map(|sent| Out::Text { rect, sent })
            }
            Op::Brightness(level) => {
                self.display.set_brightness(level).map_err(|e| e.to_string())?;
                self.brightness = level;
                Ok(Out::Done)
            }
            Op::Power(on) => {
                if on { self.display.screen_on() } else { self.display.screen_off() }.map_err(|e| e.to_string())?;
                self.power = on;
                Ok(Out::Done)
            }
            Op::Orientation(o) => {
                self.display.set_orientation(o).map_err(|e| e.to_string())?;
                self.orientation = o;
                let (w, h) = o.size();
                self.canvas = Canvas::new(w, h);
                self.flush().map(Out::Sent)
            }
        }
    }

    fn paint_image(&mut self, img: image::RgbImage, x: u16, y: u16, fit: Fit, bg: Color) {
        let (cw, ch) = (self.canvas.width() as u32, self.canvas.height() as u32);
        let (iw, ih) = (img.width(), img.height());
        let filter = image::imageops::FilterType::Triangle;
        match fit {
            Fit::None => self.canvas.draw_rgb(x, y, iw as u16, ih as u16, img.as_raw()),
            Fit::Stretch => {
                let s = image::imageops::resize(&img, cw, ch, filter);
                self.canvas.draw_rgb(0, 0, cw as u16, ch as u16, s.as_raw());
            }
            Fit::Contain => {
                let k = f64::min(cw as f64 / iw as f64, ch as f64 / ih as f64);
                let (w, h) = (((iw as f64 * k).round() as u32).max(1), ((ih as f64 * k).round() as u32).max(1));
                let s = image::imageops::resize(&img, w, h, filter);
                self.canvas.fill(bg);
                self.canvas.draw_rgb(((cw - w) / 2) as u16, ((ch - h) / 2) as u16, w as u16, h as u16, s.as_raw());
            }
            Fit::Cover => {
                let k = f64::max(cw as f64 / iw as f64, ch as f64 / ih as f64);
                let (w, h) = (((iw as f64 * k).round() as u32).max(cw), ((ih as f64 * k).round() as u32).max(ch));
                let s = image::imageops::resize(&img, w, h, filter);
                let c = image::imageops::crop_imm(&s, (w - cw) / 2, (h - ch) / 2, cw, ch).to_image();
                self.canvas.draw_rgb(0, 0, cw as u16, ch as u16, c.as_raw());
            }
        }
    }
}

/// Run the vendor wake sequence on `display`, then start the render thread.
pub fn spawn_renderer<W: Write + Send + 'static>(
    mut display: Display<W>,
    info: DeviceInfo,
    orientation: Orientation,
) -> turing_device::Result<RenderHandle> {
    display.init(orientation, 0)?;
    let (w, h) = orientation.size();
    let mut r = Renderer {
        display,
        canvas: Canvas::new(w, h),
        font: Font::default_font(),
        info,
        orientation,
        brightness: 0,
        power: true,
        connected: true,
    };
    let (tx, rx) = mpsc::channel::<(Op, Reply)>();
    thread::Builder::new()
        .name("turing-render".into())
        .spawn(move || {
            for (op, reply) in rx {
                let _ = reply.send(r.handle(op));
            }
        })
        .expect("spawn render thread");
    Ok(RenderHandle { tx })
}

fn orientation_name(o: Orientation) -> &'static str {
    match o {
        Orientation::Landscape => "landscape",
        Orientation::LandscapeFlipped => "landscape_flipped",
        Orientation::Portrait => "portrait",
        Orientation::PortraitFlipped => "portrait_flipped",
    }
}

fn parse_orientation(s: &str) -> Option<Orientation> {
    match s.to_ascii_lowercase().replace('-', "_").as_str() {
        "landscape" => Some(Orientation::Landscape),
        "landscape_flipped" => Some(Orientation::LandscapeFlipped),
        "portrait" => Some(Orientation::Portrait),
        "portrait_flipped" => Some(Orientation::PortraitFlipped),
        _ => None,
    }
}

/// Parse `#RRGGBB` (or `RRGGBB`).
pub fn parse_color(s: &str) -> Result<Color, ApiError> {
    let hex = s.strip_prefix('#').unwrap_or(s);
    let bad = || ApiError::BadRequest(format!("bad colour {s:?}, expected #RRGGBB"));
    if hex.len() != 6 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(bad());
    }
    let v = u32::from_str_radix(hex, 16).map_err(|_| bad())?;
    Ok(Color::rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

fn rect_json(r: &Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h })
}

fn rects_json(rs: &[Rect]) -> Value {
    Value::Array(rs.iter().map(rect_json).collect())
}

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Device(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            ApiError::Device(m) => (StatusCode::BAD_GATEWAY, m),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

type ApiResult = Result<Json<Value>, ApiError>;

fn sent(out: Out) -> ApiResult {
    match out {
        Out::Sent(rs) => Ok(Json(json!({ "ok": true, "sent": rects_json(&rs) }))),
        Out::Text { rect, sent } => Ok(Json(json!({ "ok": true, "rect": rect_json(&rect), "sent": rects_json(&sent) }))),
        Out::Status(v) => Ok(Json(v)),
        Out::Done => Ok(Json(json!({ "ok": true }))),
    }
}

async fn get_device(State(h): State<RenderHandle>) -> ApiResult {
    sent(h.call(Op::Status).await?)
}

#[derive(Deserialize)]
struct PowerReq {
    on: bool,
}

async fn post_power(State(h): State<RenderHandle>, Json(req): Json<PowerReq>) -> ApiResult {
    sent(h.call(Op::Power(req.on)).await?)
}

#[derive(Deserialize)]
struct BrightnessReq {
    level: u32,
}

async fn post_brightness(State(h): State<RenderHandle>, Json(req): Json<BrightnessReq>) -> ApiResult {
    let level = u8::try_from(req.level).map_err(|_| ApiError::BadRequest("level must be 0..=255 (0 = brightest)".into()))?;
    sent(h.call(Op::Brightness(level)).await?)
}

#[derive(Deserialize)]
struct OrientationReq {
    orientation: String,
}

async fn post_orientation(State(h): State<RenderHandle>, Json(req): Json<OrientationReq>) -> ApiResult {
    let o = parse_orientation(&req.orientation).ok_or_else(|| {
        ApiError::BadRequest("orientation must be landscape, landscape_flipped, portrait or portrait_flipped".into())
    })?;
    sent(h.call(Op::Orientation(o)).await?)
}

#[derive(Deserialize, Default)]
struct ClearReq {
    color: Option<String>,
}

async fn post_clear(State(h): State<RenderHandle>, Json(req): Json<ClearReq>) -> ApiResult {
    let c = match req.color {
        Some(s) => parse_color(&s)?,
        None => Color::BLACK,
    };
    sent(h.call(Op::Clear(c)).await?)
}

#[derive(Deserialize)]
struct RectReq {
    x: u16,
    y: u16,
    w: u16,
    h: u16,
}

#[derive(Deserialize)]
struct TextReq {
    x: u16,
    y: u16,
    text: String,
    #[serde(default = "default_size")]
    size: f32,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    bg: Option<String>,
    #[serde(default)]
    bg_rect: Option<RectReq>,
}

fn default_size() -> f32 {
    24.0
}

async fn post_text(State(h): State<RenderHandle>, Json(req): Json<TextReq>) -> ApiResult {
    if !(req.size > 0.0 && req.size <= 512.0) {
        return Err(ApiError::BadRequest("size must be in (0, 512]".into()));
    }
    let color = match &req.color {
        Some(s) => parse_color(s)?,
        None => Color::WHITE,
    };
    let bg = match &req.bg {
        Some(s) => Some((parse_color(s)?, req.bg_rect.as_ref().map(|r| Rect { x: r.x, y: r.y, w: r.w, h: r.h }))),
        None => None,
    };
    sent(h.call(Op::Text { x: req.x, y: req.y, text: req.text, size: req.size, color, bg }).await?)
}

#[derive(Deserialize, Default)]
struct ImageQuery {
    #[serde(default)]
    x: u16,
    #[serde(default)]
    y: u16,
    #[serde(default)]
    fit: Fit,
    #[serde(default)]
    bg: Option<String>,
}

async fn post_image(State(h): State<RenderHandle>, Query(q): Query<ImageQuery>, body: Bytes) -> ApiResult {
    let bg = match &q.bg {
        Some(s) => parse_color(s)?,
        None => Color::BLACK,
    };
    let img = tokio::task::spawn_blocking(move || image::load_from_memory(&body).map(|i| i.to_rgb8()))
        .await
        .map_err(|e| ApiError::Device(e.to_string()))?
        .map_err(|e| ApiError::BadRequest(format!("cannot decode image: {e}")))?;
    sent(h.call(Op::Image { img, x: q.x, y: q.y, fit: q.fit, bg }).await?)
}

/// Build the HTTP router around a render handle.
pub fn router(handle: RenderHandle) -> Router {
    Router::new()
        .route("/v1/device", get(get_device))
        .route("/v1/device/power", post(post_power))
        .route("/v1/device/brightness", post(post_brightness))
        .route("/v1/device/orientation", post(post_orientation))
        .route("/v1/display/clear", post(post_clear))
        .route("/v1/display/text", post(post_text))
        .route("/v1/display/image", post(post_image).layer(DefaultBodyLimit::max(32 * 1024 * 1024)))
        .with_state(handle)
}
