use std::time::Duration;
use turing_device::*;
use turing_proto::{bitmap_header, brightness, orientation, screen_on, Orientation, Rect};

// ---- discovery -------------------------------------------------------------

#[test]
fn accepts_wch_vid_with_usb35inch_serial_string() {
    assert!(is_turing_port(0x1A86, 0x5722, Some("USB35INCHIPSV2")));
}

#[test]
fn accepts_legacy_pid_f111_without_serial_string() {
    assert!(is_turing_port(0x1A86, 0xF111, None));
}

#[test]
fn rejects_other_wch_devices_such_as_ch340() {
    assert!(!is_turing_port(0x1A86, 0x7523, None));
    assert!(!is_turing_port(0x1A86, 0x55D4, Some("5715029167")));
}

#[test]
fn rejects_other_vendors_even_with_matching_serial() {
    assert!(!is_turing_port(0x0403, 0x6001, Some("USB35INCHIPSV2")));
}

#[test]
fn pacing_is_only_needed_for_pre_v2_units() {
    assert_eq!(pacing_for(Some("USB35INCHIPSV2")), None);
    assert_eq!(pacing_for(Some("USB35INCH")), Some(Duration::from_millis(10)));
    assert_eq!(pacing_for(None), Some(Duration::from_millis(10)));
}

// ---- transport over an in-memory sink ---------------------------------------

fn sink_display() -> Display<Vec<u8>> {
    Display::new(Vec::new(), None)
}

#[test]
fn blit_writes_header_then_pixels_in_order() {
    let mut d = sink_display();
    let rect = Rect { x: 20, y: 20, w: 2, h: 1 };
    d.blit(rect, &[0x00, 0xF8, 0x1F, 0x00]).unwrap();
    let mut expected = bitmap_header(rect).to_vec();
    expected.extend_from_slice(&[0x00, 0xF8, 0x1F, 0x00]);
    assert_eq!(d.sink(), &expected[..]);
}

#[test]
fn blit_rejects_pixel_buffer_of_wrong_length() {
    let mut d = sink_display();
    let err = d.blit(Rect { x: 0, y: 0, w: 2, h: 2 }, &[0u8; 6]).unwrap_err();
    assert!(matches!(err, Error::SizeMismatch { expected: 8, got: 6 }));
    assert!(d.sink().is_empty(), "nothing must be written on a rejected blit");
}

#[test]
fn blit_rejects_rect_outside_the_panel() {
    let mut d = sink_display();
    d.set_orientation(Orientation::Landscape).unwrap();
    let before = d.sink().len();
    let err = d.blit(Rect { x: 470, y: 0, w: 20, h: 1 }, &[0u8; 40]).unwrap_err();
    assert!(matches!(err, Error::OutOfBounds { .. }));
    assert_eq!(d.sink().len(), before);
}

#[test]
fn show_rgb_converts_8bit_rgb_to_rgb565_le_before_blitting() {
    let mut d = sink_display();
    let rect = Rect { x: 0, y: 0, w: 2, h: 1 };
    d.show_rgb(rect, &[255, 0, 0, 0, 0, 255]).unwrap();
    let mut expected = bitmap_header(rect).to_vec();
    expected.extend_from_slice(&[0x00, 0xF8, 0x1F, 0x00]);
    assert_eq!(d.sink(), &expected[..]);
}

#[test]
fn set_orientation_sends_121_packet_and_updates_logical_size() {
    let mut d = sink_display();
    d.set_orientation(Orientation::Portrait).unwrap();
    assert_eq!(d.sink(), &orientation(Orientation::Portrait, 320, 480)[..]);
    assert_eq!(d.size(), (320, 480));
}

#[test]
fn init_writes_screen_on_orientation_then_brightness() {
    let mut d = sink_display();
    d.init(Orientation::Landscape, 0).unwrap();
    let mut expected = screen_on().to_vec();
    expected.extend_from_slice(&orientation(Orientation::Landscape, 480, 320));
    expected.extend_from_slice(&brightness(0));
    assert_eq!(d.sink(), &expected[..]);
}

#[test]
fn default_logical_size_is_landscape_before_any_orientation_command() {
    assert_eq!(sink_display().size(), (480, 320));
}
