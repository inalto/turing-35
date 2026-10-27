use turing_canvas::{Canvas, Color};
use turing_device::Display;
use turing_proto::{bitmap_header, rgb565, Rect};

const FULL: Rect = Rect { x: 0, y: 0, w: 480, h: 320 };
const RED: Color = Color::rgb(255, 0, 0);

fn sink() -> Display<Vec<u8>> {
    Display::new(Vec::new(), None)
}

#[test]
fn first_flush_sends_the_whole_frame() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    let sent = c.flush(&mut d).unwrap();
    assert_eq!(sent, vec![FULL]);
    assert_eq!(d.sink().len(), 6 + 480 * 320 * 2);
    assert_eq!(&d.sink()[..6], &bitmap_header(FULL));
}

#[test]
fn flush_without_changes_sends_nothing() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    let before = d.sink().len();
    assert_eq!(c.flush(&mut d).unwrap(), Vec::<Rect>::new());
    assert_eq!(d.sink().len(), before);
}

#[test]
fn single_pixel_change_sends_a_1x1_rect_with_its_rgb565_value() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    let before = d.sink().len();
    c.fill_rect(Rect { x: 100, y: 50, w: 1, h: 1 }, RED);
    let sent = c.flush(&mut d).unwrap();
    assert_eq!(sent, vec![Rect { x: 100, y: 50, w: 1, h: 1 }]);
    let tail = &d.sink()[before..];
    assert_eq!(tail.len(), 6 + 2);
    assert_eq!(&tail[..6], &bitmap_header(Rect { x: 100, y: 50, w: 1, h: 1 }));
    assert_eq!(&tail[6..], &rgb565(255, 0, 0).to_le_bytes());
}

#[test]
fn changes_in_separate_row_bands_become_separate_rects() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 10, y: 10, w: 20, h: 5 }, RED);
    c.fill_rect(Rect { x: 300, y: 200, w: 40, h: 8 }, RED);
    let sent = c.flush(&mut d).unwrap();
    assert_eq!(sent, vec![Rect { x: 10, y: 10, w: 20, h: 5 }, Rect { x: 300, y: 200, w: 40, h: 8 }]);
}

#[test]
fn changes_sharing_rows_merge_into_one_rect_spanning_both() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 10, y: 10, w: 1, h: 1 }, RED);
    c.fill_rect(Rect { x: 400, y: 10, w: 1, h: 1 }, RED);
    assert_eq!(c.flush(&mut d).unwrap(), vec![Rect { x: 10, y: 10, w: 391, h: 1 }]);
}

#[test]
fn large_change_falls_back_to_a_full_frame() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 0, y: 0, w: 480, h: 160 }, RED); // 50 % of the panel
    assert_eq!(c.flush(&mut d).unwrap(), vec![FULL]);
}

#[test]
fn partial_flush_updates_the_baseline() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 5, y: 5, w: 3, h: 3 }, RED);
    c.flush(&mut d).unwrap();
    assert_eq!(c.flush(&mut d).unwrap(), Vec::<Rect>::new());
}

#[test]
fn flush_rejects_a_canvas_that_does_not_match_the_display_size() {
    let mut c = Canvas::new(100, 100);
    let mut d = sink(); // display is 480x320 landscape by default
    assert!(c.flush(&mut d).is_err());
}

#[test]
fn flush_sends_only_the_changed_rect_pixels_in_row_major_order() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    let before = d.sink().len();
    c.fill_rect(Rect { x: 2, y: 3, w: 2, h: 2 }, RED);
    c.flush(&mut d).unwrap();
    let tail = &d.sink()[before + 6..];
    let red = rgb565(255, 0, 0).to_le_bytes();
    assert_eq!(tail, [red, red, red, red].concat());
}
