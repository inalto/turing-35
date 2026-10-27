use turing_canvas::{Canvas, Color, Font};
use turing_proto::Rect;

const BLACK: Color = Color::rgb(0, 0, 0);
const WHITE: Color = Color::rgb(255, 255, 255);
const RED: Color = Color::rgb(255, 0, 0);

fn count_pixels(c: &Canvas, pred: impl Fn(Color) -> bool) -> usize {
    (0..c.height()).flat_map(|y| (0..c.width()).map(move |x| (x, y))).filter(|&(x, y)| pred(c.pixel(x, y))).count()
}

#[test]
fn new_canvas_is_landscape_and_black() {
    let c = Canvas::landscape();
    assert_eq!((c.width(), c.height()), (480, 320));
    assert_eq!(c.as_rgb().len(), 480 * 320 * 3);
    assert!(c.as_rgb().iter().all(|&b| b == 0));
}

#[test]
fn fill_sets_every_pixel() {
    let mut c = Canvas::new(4, 3);
    c.fill(RED);
    assert_eq!(count_pixels(&c, |p| p == RED), 12);
}

#[test]
fn fill_rect_touches_only_the_rect_and_clips_at_the_edge() {
    let mut c = Canvas::new(10, 10);
    c.fill_rect(Rect { x: 8, y: 8, w: 5, h: 5 }, WHITE);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 4);
    assert_eq!(c.pixel(7, 7), BLACK);
    assert_eq!(c.pixel(9, 9), WHITE);
}

#[test]
fn draw_rgb_copies_pixels_at_offset_and_clips_overhang() {
    let mut c = Canvas::new(4, 4);
    // 3x2 image: red row, white row
    let img = [255, 0, 0, 255, 0, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255, 255, 255, 255];
    c.draw_rgb(2, 3, 3, 2, &img);
    assert_eq!(c.pixel(2, 3), RED);
    assert_eq!(c.pixel(3, 3), RED);
    assert_eq!(count_pixels(&c, |p| p == RED), 2);
    assert_eq!(count_pixels(&c, |p| p == WHITE), 0, "second row is below the canvas");
    assert_eq!(c.pixel(1, 3), BLACK);
}

#[test]
fn draw_text_paints_inside_the_returned_rect_only() {
    let mut c = Canvas::landscape();
    let font = Font::default_font();
    let r = c.draw_text(20, 40, "Hi", &font, 32.0, WHITE);
    assert!(r.w > 0 && r.h > 0);
    assert!(r.x >= 20 && r.y >= 40);
    assert!(r.x + r.w <= 480 && r.y + r.h <= 320);
    let inside = (r.x..r.x + r.w).flat_map(|x| (r.y..r.y + r.h).map(move |y| (x, y)));
    let lit_inside = inside.filter(|&(x, y)| c.pixel(x, y) != BLACK).count();
    assert!(lit_inside > 20, "glyphs should light pixels inside the rect, got {lit_inside}");
    let lit_total = count_pixels(&c, |p| p != BLACK);
    assert_eq!(lit_total, lit_inside, "nothing may be painted outside the returned rect");
}

#[test]
fn draw_text_is_anti_aliased_against_the_background() {
    let mut c = Canvas::landscape();
    c.fill(Color::rgb(0, 0, 200));
    let font = Font::default_font();
    c.draw_text(10, 10, "O", &font, 40.0, WHITE);
    // expect at least one blended pixel: neither pure background nor pure foreground
    let blended = count_pixels(&c, |p| p != Color::rgb(0, 0, 200) && p != WHITE);
    assert!(blended > 0);
}

#[test]
fn larger_pixel_size_gives_taller_text() {
    let mut a = Canvas::landscape();
    let mut b = Canvas::landscape();
    let font = Font::default_font();
    let small = a.draw_text(0, 0, "Ag", &font, 16.0, WHITE);
    let large = b.draw_text(0, 0, "Ag", &font, 48.0, WHITE);
    assert!(large.h > small.h && large.w > small.w);
}

#[test]
fn measure_text_width_grows_with_the_string() {
    let font = Font::default_font();
    let (w1, h1) = turing_canvas::measure_text("a", &font, 24.0);
    let (w2, h2) = turing_canvas::measure_text("aaaa", &font, 24.0);
    assert!(w2 > w1 * 3);
    assert_eq!(h1, h2, "line height does not depend on the glyphs");
}

#[test]
fn text_clips_at_the_canvas_edge_without_panicking() {
    let mut c = Canvas::new(30, 20);
    let font = Font::default_font();
    let r = c.draw_text(20, 5, "Overflow", &font, 20.0, WHITE);
    assert!(r.x + r.w <= 30 && r.y + r.h <= 20);
    assert!(count_pixels(&c, |p| p != BLACK) > 0);
}

#[test]
fn font_can_be_loaded_from_bytes() {
    let bytes = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/fonts/Roboto-Regular.ttf")).unwrap();
    assert!(Font::from_bytes(&bytes).is_ok());
    assert!(Font::from_bytes(b"not a font").is_err());
}
