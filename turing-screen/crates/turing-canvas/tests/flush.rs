use turing_canvas::{Canvas, Color, DIRTY_GAP_PX};
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
fn distant_changes_sharing_rows_become_separate_rects() {
    // 10 and 400 are 389 unchanged columns apart: far more than DIRTY_GAP_PX.
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 10, y: 10, w: 1, h: 1 }, RED);
    c.fill_rect(Rect { x: 400, y: 10, w: 1, h: 1 }, RED);
    assert_eq!(c.flush(&mut d).unwrap(), vec![Rect { x: 10, y: 10, w: 1, h: 1 }, Rect { x: 400, y: 10, w: 1, h: 1 }]);
}

#[test]
fn distant_widgets_on_overlapping_rows_are_sent_separately() {
    // Two widgets side by side, with different heights, as in a two-column layout.
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 12, y: 60, w: 120, h: 50 }, RED);
    c.fill_rect(Rect { x: 252, y: 80, w: 216, h: 14 }, RED);
    assert_eq!(
        c.flush(&mut d).unwrap(),
        vec![Rect { x: 12, y: 60, w: 120, h: 50 }, Rect { x: 252, y: 80, w: 216, h: 14 }]
    );
}

#[test]
fn changes_up_to_the_gap_threshold_apart_merge_into_one_rect() {
    let gap = DIRTY_GAP_PX;
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    // Exactly DIRTY_GAP_PX unchanged columns between the two pixels: still one rect.
    c.fill_rect(Rect { x: 10, y: 10, w: 1, h: 1 }, RED);
    c.fill_rect(Rect { x: 11 + gap, y: 10, w: 1, h: 1 }, RED);
    assert_eq!(c.flush(&mut d).unwrap(), vec![Rect { x: 10, y: 10, w: gap + 2, h: 1 }]);
    // One column more and they are sent separately.
    c.fill_rect(Rect { x: 100, y: 50, w: 1, h: 1 }, RED);
    c.fill_rect(Rect { x: 102 + gap, y: 50, w: 1, h: 1 }, RED);
    assert_eq!(
        c.flush(&mut d).unwrap(),
        vec![Rect { x: 100, y: 50, w: 1, h: 1 }, Rect { x: 102 + gap, y: 50, w: 1, h: 1 }]
    );
}

#[test]
fn same_columns_split_by_unchanged_rows_stay_separate_when_bridged_elsewhere() {
    // A and B share columns but are 15 rows apart; C, far to the right,
    // spans the rows of both. A and B must not merge through C's rows.
    let (a, b, c_rect) = (
        Rect { x: 10, y: 10, w: 20, h: 5 },
        Rect { x: 10, y: 30, w: 20, h: 5 },
        Rect { x: 300, y: 10, w: 10, h: 25 },
    );
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    for r in [a, b, c_rect] {
        c.fill_rect(r, RED);
    }
    assert_eq!(c.flush(&mut d).unwrap(), vec![a, c_rect, b]);
}

#[test]
fn a_single_unchanged_row_does_not_split_but_two_do() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 10, y: 10, w: 5, h: 1 }, RED);
    c.fill_rect(Rect { x: 10, y: 12, w: 5, h: 1 }, RED);
    assert_eq!(c.flush(&mut d).unwrap(), vec![Rect { x: 10, y: 10, w: 5, h: 3 }]);
    c.fill_rect(Rect { x: 100, y: 100, w: 5, h: 1 }, RED);
    c.fill_rect(Rect { x: 100, y: 103, w: 5, h: 1 }, RED);
    assert_eq!(
        c.flush(&mut d).unwrap(),
        vec![Rect { x: 100, y: 100, w: 5, h: 1 }, Rect { x: 100, y: 103, w: 5, h: 1 }]
    );
}

#[test]
fn split_rects_carry_exactly_their_own_pixels() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    let before = d.sink().len();
    c.fill_rect(Rect { x: 10, y: 10, w: 1, h: 1 }, RED);
    c.fill_rect(Rect { x: 400, y: 10, w: 1, h: 1 }, Color::WHITE);
    c.flush(&mut d).unwrap();
    let tail = &d.sink()[before..];
    let expected = [
        bitmap_header(Rect { x: 10, y: 10, w: 1, h: 1 }).to_vec(),
        rgb565(255, 0, 0).to_le_bytes().to_vec(),
        bitmap_header(Rect { x: 400, y: 10, w: 1, h: 1 }).to_vec(),
        rgb565(255, 255, 255).to_le_bytes().to_vec(),
    ]
    .concat();
    assert_eq!(tail, expected);
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

#[test]
fn flush_after_invalidate_sends_the_whole_frame_again() {
    let mut c = Canvas::landscape();
    let mut d = sink();
    c.flush(&mut d).unwrap();
    c.fill_rect(Rect { x: 5, y: 5, w: 3, h: 3 }, RED);
    c.invalidate();
    assert_eq!(c.flush(&mut d).unwrap(), vec![FULL]);
    assert_eq!(c.flush(&mut d).unwrap(), Vec::<Rect>::new());
}
