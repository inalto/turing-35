//! Byte-exact tests. Vectors come from docs/PROTOCOL.md §2 and were verified on the real panel.
use turing_proto::*;

#[test]
fn header_packs_full_landscape_frame_rect() {
    // (197, 0, 0, 479, 319) -> 00 00 07 7D 3F C5
    assert_eq!(header(Cmd::Bitmap, 0, 0, 479, 319), [0x00, 0x00, 0x07, 0x7D, 0x3F, 0xC5]);
}

#[test]
fn header_packs_offset_rect() {
    // (197, 20, 20, 119, 69) -> 05 01 41 DC 45 C5
    assert_eq!(header(Cmd::Bitmap, 20, 20, 119, 69), [0x05, 0x01, 0x41, 0xDC, 0x45, 0xC5]);
}

#[test]
fn header_packs_arc_pixels_rect() {
    // (195, 300, 150, 56, 0) -> 4B 09 60 E0 00 C3
    assert_eq!(header(Cmd::DrawPixels, 300, 150, 56, 0), [0x4B, 0x09, 0x60, 0xE0, 0x00, 0xC3]);
}

#[test]
fn control_commands_are_header_only_with_zero_geometry() {
    assert_eq!(screen_on(), [0, 0, 0, 0, 0, 109]);
    assert_eq!(screen_off(), [0, 0, 0, 0, 0, 108]);
    assert_eq!(clear(), [0, 0, 0, 0, 0, 102]);
    assert_eq!(probe(), [0, 0, 0, 0, 0, 255]);
}

#[test]
fn brightness_level_travels_in_x_field() {
    // vendor sends 110 with x = level; level 255 -> b0 = 255 >> 2 = 0x3F, b1 = (255 & 3) << 6 = 0xC0
    assert_eq!(brightness(255), [0x3F, 0xC0, 0x00, 0x00, 0x00, 110]);
    assert_eq!(brightness(0), [0, 0, 0, 0, 0, 110]);
}

#[test]
fn orientation_packet_is_16_bytes_with_100_plus_value_and_big_endian_size() {
    let p = orientation(Orientation::Landscape, 480, 320);
    assert_eq!(p.len(), 16);
    assert_eq!(&p[..6], &[0, 0, 0, 0, 0, 121]);
    assert_eq!(p[6], 102);
    assert_eq!(&p[7..11], &[0x01, 0xE0, 0x01, 0x40]);
    assert!(p[11..].iter().all(|&b| b == 0));
}

#[test]
fn orientation_values_match_vendor_enumeration() {
    assert_eq!(orientation(Orientation::Portrait, 320, 480)[6], 100);
    assert_eq!(orientation(Orientation::PortraitFlipped, 320, 480)[6], 101);
    assert_eq!(orientation(Orientation::LandscapeFlipped, 480, 320)[6], 103);
}

#[test]
fn orientation_knows_its_logical_size() {
    assert_eq!(Orientation::Landscape.size(), (480, 320));
    assert_eq!(Orientation::LandscapeFlipped.size(), (480, 320));
    assert_eq!(Orientation::Portrait.size(), (320, 480));
    assert_eq!(Orientation::PortraitFlipped.size(), (320, 480));
}

#[test]
fn mirror_packet_is_16_bytes_with_flag_at_index_6() {
    let p = mirror(true);
    assert_eq!(p.len(), 16);
    assert_eq!(&p[..6], &[0, 0, 0, 0, 0, 122]);
    assert_eq!(p[6], 1);
    assert_eq!(mirror(false)[6], 0);
}

#[test]
fn bitmap_header_uses_inclusive_end_coordinates() {
    assert_eq!(bitmap_header(Rect { x: 0, y: 0, w: 480, h: 320 }), header(Cmd::Bitmap, 0, 0, 479, 319));
    assert_eq!(bitmap_header(Rect { x: 20, y: 20, w: 100, h: 50 }), header(Cmd::Bitmap, 20, 20, 119, 69));
}

#[test]
fn rgb565_packs_channels_like_the_vendor_formula() {
    // ((R<<8)&0xF800) | ((G<<3)&0x7E0) | (B>>3)
    assert_eq!(rgb565(255, 255, 255), 0xFFFF);
    assert_eq!(rgb565(0, 0, 0), 0x0000);
    assert_eq!(rgb565(255, 0, 0), 0xF800);
    assert_eq!(rgb565(0, 255, 0), 0x07E0);
    assert_eq!(rgb565(0, 0, 255), 0x001F);
    assert_eq!(rgb565(0x08, 0x04, 0x08), 0x0821); // lowest bit of each channel
}

#[test]
fn encode_rgb_writes_little_endian_u16_per_pixel() {
    let rgb = [255u8, 0, 0, 0, 0, 255];
    let mut out = [0u8; 4];
    encode_rgb(&rgb, &mut out);
    assert_eq!(out, [0x00, 0xF8, 0x1F, 0x00]);
}

#[test]
fn rect_byte_len_is_two_bytes_per_pixel() {
    assert_eq!(Rect { x: 0, y: 0, w: 480, h: 320 }.byte_len(), 307_200);
}
