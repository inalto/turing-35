use turing_canvas::{Canvas, Color};
use turing_proto::Rect;

const BLACK: Color = Color::rgb(0, 0, 0);
const WHITE: Color = Color::rgb(255, 255, 255);
const RED: Color = Color::rgb(255, 0, 0);

fn count_pixels(c: &Canvas, pred: impl Fn(Color) -> bool) -> usize {
    (0..c.height()).flat_map(|y| (0..c.width()).map(move |x| (x, y))).filter(|&(x, y)| pred(c.pixel(x, y))).count()
}

// ---- blend_rect ----

#[test]
fn blend_rect_with_full_alpha_equals_fill_rect() {
    let r = Rect { x: 2, y: 3, w: 5, h: 4 };
    let mut a = Canvas::new(10, 10);
    let mut b = Canvas::new(10, 10);
    a.blend_rect(r, RED, 255);
    b.fill_rect(r, RED);
    assert_eq!(a.as_rgb(), b.as_rgb());
}

#[test]
fn blend_rect_with_zero_alpha_leaves_pixels_untouched() {
    let mut c = Canvas::new(10, 10);
    c.fill(Color::rgb(10, 20, 30));
    c.blend_rect(Rect { x: 0, y: 0, w: 10, h: 10 }, WHITE, 0);
    assert_eq!(count_pixels(&c, |p| p == Color::rgb(10, 20, 30)), 100);
}

#[test]
fn blend_rect_half_alpha_of_white_over_black_is_mid_grey() {
    let mut c = Canvas::new(4, 4);
    c.blend_rect(Rect { x: 1, y: 1, w: 2, h: 2 }, WHITE, 128);
    assert_eq!(c.pixel(1, 1), Color::rgb(128, 128, 128));
    assert_eq!(c.pixel(2, 2), Color::rgb(128, 128, 128));
    assert_eq!(c.pixel(0, 0), BLACK);
    assert_eq!(count_pixels(&c, |p| p != BLACK), 4);
}

#[test]
fn blend_rect_clips_at_the_edge() {
    let mut c = Canvas::new(10, 10);
    c.blend_rect(Rect { x: 8, y: 8, w: 5, h: 5 }, WHITE, 128);
    assert_eq!(count_pixels(&c, |p| p != BLACK), 4);
    assert_eq!(c.pixel(9, 9), Color::rgb(128, 128, 128));
    assert_eq!(c.pixel(7, 7), BLACK);
}

// ---- draw_line ----

#[test]
fn horizontal_line_paints_abs_dx_plus_one_pixels_in_its_row() {
    let mut c = Canvas::new(20, 10);
    c.draw_line(3, 4, 12, 4, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 10);
    assert!((3..=12).all(|x| c.pixel(x, 4) == WHITE));
}

#[test]
fn reversed_horizontal_line_paints_the_same_pixels() {
    let mut c = Canvas::new(20, 10);
    c.draw_line(12, 4, 3, 4, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 10);
    assert!((3..=12).all(|x| c.pixel(x, 4) == WHITE));
}

#[test]
fn vertical_line_paints_abs_dy_plus_one_pixels_in_its_column() {
    let mut c = Canvas::new(10, 20);
    c.draw_line(5, 2, 5, 9, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 8);
    assert!((2..=9).all(|y| c.pixel(5, y) == WHITE));
}

#[test]
fn diagonal_line_paints_exactly_the_diagonal() {
    let mut c = Canvas::new(10, 10);
    c.draw_line(0, 0, 5, 5, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 6);
    assert!((0..=5).all(|i| c.pixel(i, i) == WHITE));
}

#[test]
fn line_endpoints_are_inclusive() {
    let mut c = Canvas::new(10, 10);
    c.draw_line(1, 2, 7, 5, RED);
    assert_eq!(c.pixel(1, 2), RED);
    assert_eq!(c.pixel(7, 5), RED);
}

#[test]
fn line_starting_off_canvas_paints_only_visible_pixels_without_panicking() {
    let mut c = Canvas::new(10, 10);
    c.draw_line(-5, -5, 3, 3, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 4);
    assert!((0..=3).all(|i| c.pixel(i, i) == WHITE));
}

#[test]
fn line_ending_beyond_the_canvas_is_clipped() {
    let mut c = Canvas::new(10, 10);
    c.draw_line(7, 7, 100, 100, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 3);
}

// ---- draw_rect_outline ----

#[test]
fn outline_of_10x10_paints_36_pixels_and_leaves_the_interior() {
    let mut c = Canvas::new(20, 20);
    c.draw_rect_outline(Rect { x: 2, y: 3, w: 10, h: 10 }, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 36);
    assert_eq!(c.pixel(2, 3), WHITE);
    assert_eq!(c.pixel(11, 12), WHITE);
    for y in 4..12 {
        for x in 3..11 {
            assert_eq!(c.pixel(x, y), BLACK, "interior ({x},{y}) must be untouched");
        }
    }
}

#[test]
fn outline_partly_off_canvas_paints_only_visible_edges() {
    let mut c = Canvas::new(10, 10);
    // Right and bottom edges lie beyond the canvas; only the top and left edges are visible.
    c.draw_rect_outline(Rect { x: 6, y: 6, w: 10, h: 10 }, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 4 + 4 - 1);
    assert_eq!(c.pixel(6, 6), WHITE);
    assert_eq!(c.pixel(9, 6), WHITE);
    assert_eq!(c.pixel(6, 9), WHITE);
    assert_eq!(c.pixel(9, 9), BLACK);
}

#[test]
fn outline_of_a_one_pixel_rect_is_that_pixel() {
    let mut c = Canvas::new(5, 5);
    c.draw_rect_outline(Rect { x: 2, y: 2, w: 1, h: 1 }, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 1);
}

#[test]
fn outline_of_an_empty_rect_paints_nothing() {
    let mut c = Canvas::new(5, 5);
    c.draw_rect_outline(Rect { x: 2, y: 2, w: 0, h: 3 }, WHITE);
    assert_eq!(count_pixels(&c, |p| p != BLACK), 0);
}

// ---- blit_from ----

#[test]
fn blit_from_copies_exactly_the_rect() {
    let mut src = Canvas::new(20, 10);
    src.fill(RED);
    let mut dst = Canvas::new(20, 10);
    dst.blit_from(&src, Rect { x: 4, y: 2, w: 6, h: 3 });
    assert_eq!(count_pixels(&dst, |p| p == RED), 18);
    assert_eq!(dst.pixel(4, 2), RED);
    assert_eq!(dst.pixel(9, 4), RED);
    assert_eq!(dst.pixel(10, 4), BLACK);
}

#[test]
fn blit_from_clips_at_the_edge() {
    let mut src = Canvas::new(10, 10);
    src.fill(RED);
    let mut dst = Canvas::new(10, 10);
    dst.blit_from(&src, Rect { x: 8, y: 8, w: 5, h: 5 });
    assert_eq!(count_pixels(&dst, |p| p == RED), 4);
}

#[test]
#[should_panic(expected = "same size")]
fn blit_from_panics_on_size_mismatch() {
    let src = Canvas::new(10, 10);
    let mut dst = Canvas::new(10, 11);
    dst.blit_from(&src, Rect { x: 0, y: 0, w: 1, h: 1 });
}
