//! End-to-end tests of the HTTP API against an in-memory display sink.
//! Every request goes through the real router, render thread, canvas and
//! Display; only the serial port is replaced by a shared byte buffer.
use std::io::Write;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use turing_daemon::{router, spawn_renderer, DeviceInfo};
use turing_device::Display;
use turing_proto::{bitmap_header, brightness, orientation, rgb565, screen_off, screen_on, Orientation, Rect};

/// A `Write` sink the test can read back after the render thread wrote to it.
#[derive(Clone, Default)]
struct SharedSink(Arc<Mutex<Vec<u8>>>);

impl Write for SharedSink {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl SharedSink {
    fn bytes(&self) -> Vec<u8> {
        self.0.lock().unwrap().clone()
    }
}

const FULL: Rect = Rect { x: 0, y: 0, w: 480, h: 320 };

fn harness() -> (Router, SharedSink) {
    let sink = SharedSink::default();
    let display = Display::new(sink.clone(), None);
    let info = DeviceInfo { port: "COMTEST".into(), serial: Some("USB35INCHIPSV2".into()) };
    let handle = spawn_renderer(display, info, Orientation::Landscape).expect("init");
    (router(handle), sink)
}

async fn call(app: &Router, method: &str, path: &str, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    let body = match body {
        Some(v) => {
            req = req.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let resp = app.clone().oneshot(req.body(body).unwrap()).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json = if bytes.is_empty() { Value::Null } else { serde_json::from_slice(&bytes).unwrap() };
    (status, json)
}

async fn post_bytes(app: &Router, path: &str, content_type: &str, body: Vec<u8>) -> (StatusCode, Value) {
    let req = Request::builder().method("POST").uri(path).header("content-type", content_type).body(Body::from(body)).unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

fn rect_json(r: Rect) -> Value {
    json!({ "x": r.x, "y": r.y, "w": r.w, "h": r.h })
}

// ---- startup ----------------------------------------------------------------

#[test]
fn spawning_the_renderer_runs_the_wake_sequence() {
    let (_app, sink) = harness();
    let mut expected = screen_on().to_vec();
    expected.extend_from_slice(&orientation(Orientation::Landscape, 480, 320));
    expected.extend_from_slice(&brightness(0));
    assert_eq!(sink.bytes(), expected);
}

#[tokio::test]
async fn device_status_reports_port_size_and_state() {
    let (app, _) = harness();
    let (status, body) = call(&app, "GET", "/v1/device", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["port"], "COMTEST");
    assert_eq!(body["serial"], "USB35INCHIPSV2");
    assert_eq!(body["connected"], true);
    assert_eq!(body["orientation"], "landscape");
    assert_eq!(body["brightness"], 0);
    assert_eq!(body["power"], true);
    assert_eq!(body["size"], json!({ "w": 480, "h": 320 }));
}

// ---- display ----------------------------------------------------------------

#[tokio::test]
async fn clear_paints_a_full_frame_of_the_requested_colour() {
    let (app, sink) = harness();
    let before = sink.bytes().len();
    let (status, body) = call(&app, "POST", "/v1/display/clear", Some(json!({ "color": "#FF0000" }))).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["sent"], json!([rect_json(FULL)]));
    let tail = &sink.bytes()[before..];
    assert_eq!(&tail[..6], &bitmap_header(FULL));
    assert_eq!(tail.len(), 6 + FULL.byte_len());
    let red = rgb565(255, 0, 0).to_le_bytes();
    assert!(tail[6..].chunks_exact(2).all(|p| p == red));
}

#[tokio::test]
async fn clear_defaults_to_black() {
    let (app, sink) = harness();
    let before = sink.bytes().len();
    let (status, _) = call(&app, "POST", "/v1/display/clear", Some(json!({}))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(sink.bytes()[before + 6..].iter().all(|&b| b == 0));
}

#[tokio::test]
async fn text_returns_the_painted_rect_and_later_updates_are_partial() {
    let (app, sink) = harness();
    let (status, body) =
        call(&app, "POST", "/v1/display/text", Some(json!({ "x": 20, "y": 40, "text": "Hi", "size": 32, "color": "#FFFFFF" }))).await;
    assert_eq!(status, StatusCode::OK);
    let r = &body["rect"];
    assert!(r["w"].as_u64().unwrap() > 0 && r["h"].as_u64().unwrap() > 0);
    assert_eq!(body["sent"], json!([rect_json(FULL)]), "first paint is a full frame");

    let before = sink.bytes().len();
    let (status, body) =
        call(&app, "POST", "/v1/display/text", Some(json!({ "x": 20, "y": 40, "text": "Hi", "size": 32, "color": "#FF0000" }))).await;
    assert_eq!(status, StatusCode::OK);
    let sent = body["sent"].as_array().unwrap();
    assert_eq!(sent.len(), 1);
    assert!(sent[0]["w"].as_u64().unwrap() < 480, "recolouring text must not resend the full frame");
    assert!(sink.bytes().len() - before < 6 + FULL.byte_len() / 10);
}

#[tokio::test]
async fn text_with_background_paints_the_box_first() {
    let (app, _) = harness();
    let (status, body) = call(
        &app,
        "POST",
        "/v1/display/text",
        Some(json!({ "x": 0, "y": 0, "text": "Hi", "size": 24, "color": "#000000", "bg": "#00FF00", "bg_rect": { "x": 0, "y": 0, "w": 100, "h": 40 } })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["rect"]["w"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn image_png_is_decoded_and_shown_at_origin_with_fit_none() {
    let (app, sink) = harness();
    let mut png = Vec::new();
    let img = image::RgbImage::from_pixel(480, 320, image::Rgb([0, 255, 0]));
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let before = sink.bytes().len();
    let (status, body) = post_bytes(&app, "/v1/display/image?fit=none", "image/png", png).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sent"], json!([rect_json(FULL)]));
    let green = rgb565(0, 255, 0).to_le_bytes();
    assert!(sink.bytes()[before + 6..].chunks_exact(2).all(|p| p == green));
}

#[tokio::test]
async fn image_fit_contain_letterboxes_on_the_background_colour() {
    let (app, sink) = harness();
    let mut png = Vec::new();
    // 2:1 image on a 3:2 panel -> bars top and bottom
    let img = image::RgbImage::from_pixel(200, 100, image::Rgb([255, 255, 255]));
    image::DynamicImage::ImageRgb8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let before = sink.bytes().len();
    let (status, _) = post_bytes(&app, "/v1/display/image?fit=contain&bg=%230000FF", "image/png", png).await;
    assert_eq!(status, StatusCode::OK);
    let frame = &sink.bytes()[before + 6..];
    let px = |x: usize, y: usize| u16::from_le_bytes([frame[(y * 480 + x) * 2], frame[(y * 480 + x) * 2 + 1]]);
    assert_eq!(px(240, 2), rgb565(0, 0, 255), "top bar is background");
    assert_eq!(px(240, 160), rgb565(255, 255, 255), "centre is the image");
    assert_eq!(px(240, 317), rgb565(0, 0, 255), "bottom bar is background");
}

#[tokio::test]
async fn image_with_undecodable_body_is_a_bad_request() {
    let (app, _) = harness();
    let (status, body) = post_bytes(&app, "/v1/display/image", "image/png", b"definitely not a png".to_vec()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().len() > 3);
}

// ---- device control ---------------------------------------------------------

#[tokio::test]
async fn brightness_is_sent_and_reported() {
    let (app, sink) = harness();
    let (status, _) = call(&app, "POST", "/v1/device/brightness", Some(json!({ "level": 10 }))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(sink.bytes().ends_with(&brightness(10)));
    let (_, body) = call(&app, "GET", "/v1/device", None).await;
    assert_eq!(body["brightness"], 10);
}

#[tokio::test]
async fn brightness_out_of_range_is_rejected() {
    let (app, _) = harness();
    let (status, _) = call(&app, "POST", "/v1/device/brightness", Some(json!({ "level": 300 }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn power_off_sends_screen_off_and_power_on_sends_screen_on() {
    let (app, sink) = harness();
    let (status, _) = call(&app, "POST", "/v1/device/power", Some(json!({ "on": false }))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(sink.bytes().ends_with(&screen_off()));
    let (_, body) = call(&app, "GET", "/v1/device", None).await;
    assert_eq!(body["power"], false);
    call(&app, "POST", "/v1/device/power", Some(json!({ "on": true }))).await;
    assert!(sink.bytes().ends_with(&screen_on()));
}

#[tokio::test]
async fn orientation_change_sends_121_resizes_canvas_and_redraws() {
    let (app, sink) = harness();
    let before = sink.bytes().len();
    let (status, body) = call(&app, "POST", "/v1/device/orientation", Some(json!({ "orientation": "portrait" }))).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let tail = &sink.bytes()[before..];
    assert_eq!(&tail[..16], &orientation(Orientation::Portrait, 320, 480));
    assert_eq!(&tail[16..22], &bitmap_header(Rect { x: 0, y: 0, w: 320, h: 480 }), "full redraw at the new size");
    let (_, body) = call(&app, "GET", "/v1/device", None).await;
    assert_eq!(body["orientation"], "portrait");
    assert_eq!(body["size"], json!({ "w": 320, "h": 480 }));
}

#[tokio::test]
async fn unknown_orientation_is_rejected() {
    let (app, _) = harness();
    let (status, _) = call(&app, "POST", "/v1/device/orientation", Some(json!({ "orientation": "diagonal" }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn malformed_colour_is_rejected_without_touching_the_display() {
    let (app, sink) = harness();
    let before = sink.bytes().len();
    let (status, body) = call(&app, "POST", "/v1/display/clear", Some(json!({ "color": "red-ish" }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].is_string());
    assert_eq!(sink.bytes().len(), before);
}
