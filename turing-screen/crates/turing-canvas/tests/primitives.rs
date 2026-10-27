use turing_canvas::{line_points, Canvas, Color};
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

// ---- line_points ----

#[test]
fn line_points_run_from_start_to_end_inclusive() {
    let pts: Vec<(i32, i32)> = line_points(1, 2, 7, 5).collect();
    assert_eq!(pts.first(), Some(&(1, 2)));
    assert_eq!(pts.last(), Some(&(7, 5)));
    assert_eq!(pts.len(), 7, "one point per column of a shallow line");
}

#[test]
fn line_points_of_a_steep_reversed_line_step_one_pixel_at_a_time() {
    let pts: Vec<(i32, i32)> = line_points(4, 9, 2, 1).collect();
    assert_eq!(pts.len(), 9, "one point per row of a steep line");
    assert_eq!((pts[0], pts[8]), ((4, 9), (2, 1)));
    for w in pts.windows(2) {
        assert!((w[1].0 - w[0].0).abs() <= 1 && w[1].1 - w[0].1 == -1, "{w:?}");
    }
}

#[test]
fn line_points_of_a_degenerate_line_is_one_point() {
    assert_eq!(line_points(3, 3, 3, 3).collect::<Vec<_>>(), vec![(3, 3)]);
}

#[test]
fn line_points_include_off_canvas_coordinates() {
    assert_eq!(line_points(-2, -2, 0, 0).collect::<Vec<_>>(), vec![(-2, -2), (-1, -1), (0, 0)]);
}

#[test]
fn draw_line_paints_exactly_the_line_points() {
    for (x0, y0, x1, y1) in [(1, 2, 17, 9), (15, 1, 2, 18), (0, 19, 19, 0), (3, 3, 3, 3), (-4, 6, 25, 11)] {
        let mut c = Canvas::new(20, 20);
        c.draw_line(x0, y0, x1, y1, WHITE);
        let mut expected = Canvas::new(20, 20);
        for (x, y) in line_points(x0, y0, x1, y1).filter(|&(x, y)| (0..20).contains(&x) && (0..20).contains(&y)) {
            expected.fill_rect(Rect { x: x as u16, y: y as u16, w: 1, h: 1 }, WHITE);
        }
        assert_eq!(c.as_rgb(), expected.as_rgb(), "line ({x0},{y0})-({x1},{y1})");
    }
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
