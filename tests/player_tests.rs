use termstream::clock::Clock;
use termstream::render::ascii::render_ascii;
use termstream::render::braille::render_braille;
use termstream::render::half_block::render_half_block;
use termstream::render::{ColorLut, RenderGeometry};
use termstream::types::{ColorFilter, RenderMode, ScalingMode, VideoFrame};
use termstream::ui::format_time;

#[test]
fn test_format_time() {
    assert_eq!(format_time(0.0), "00:00");
    assert_eq!(format_time(9.4), "00:09");
    assert_eq!(format_time(65.0), "01:05");
    assert_eq!(format_time(3600.0), "01:00:00");
    assert_eq!(format_time(3665.0), "01:01:05");
}

#[test]
fn test_geometry_fit_and_stretch() {
    // 80x24 terminal, 16:9 video (e.g. 1920x1080)
    let geom_fit = RenderGeometry::calculate(80, 24, 1920, 1080, ScalingMode::Fit, 1, 2);
    assert!(geom_fit.active_cols <= 80);
    assert!(geom_fit.active_rows <= 24);
    assert!(geom_fit.pixel_width > 0);
    assert!(geom_fit.pixel_height > 0);

    let geom_stretch = RenderGeometry::calculate(80, 24, 1920, 1080, ScalingMode::Stretch, 1, 2);
    assert_eq!(geom_stretch.active_cols, 80);
    assert_eq!(geom_stretch.active_rows, 24);
    assert_eq!(geom_stretch.pixel_width, 80);
    assert_eq!(geom_stretch.pixel_height, 48); // 24 rows * 2 pixels_per_row
}

#[test]
fn test_color_lut_transformations() {
    let normal_lut = ColorLut::new(0, 1.0, false, ColorFilter::None);
    assert_eq!(normal_lut.transform_rgb(100, 150, 200), (100, 150, 200));

    let invert_lut = ColorLut::new(0, 1.0, true, ColorFilter::None);
    assert_eq!(invert_lut.transform_rgb(0, 100, 255), (255, 155, 0));

    let gray_lut = ColorLut::new(0, 1.0, false, ColorFilter::Grayscale);
    let (r, g, b) = gray_lut.transform_rgb(100, 100, 100);
    assert_eq!(r, g);
    assert_eq!(g, b);

    let green_lut = ColorLut::new(0, 1.0, false, ColorFilter::MatrixGreen);
    let (gr, gg, gb) = green_lut.transform_rgb(200, 200, 200);
    assert!(gg > gr);
    assert!(gg > gb);
}

#[test]
fn test_render_half_block_output() {
    let geom = RenderGeometry::calculate(10, 5, 10, 10, ScalingMode::Stretch, 1, 2);
    let lut = ColorLut::new(0, 1.0, false, ColorFilter::None);

    // 10x10 frame (300 bytes)
    let mut data = vec![0u8; 10 * 10 * 3];
    // Fill top pixel with red, bottom with blue
    data[0] = 255; // R
    data[10 * 3 + 2] = 255; // B in row 1

    let frame = VideoFrame {
        width: 10,
        height: 10,
        pts_secs: 0.0,
        data,
    };

    let mut buffer = String::new();
    render_half_block(&frame, &geom, &lut, &mut buffer);

    assert!(!buffer.is_empty());
    assert!(buffer.contains('▀'));
    assert!(buffer.contains("\x1b[38;2;"));
    assert!(buffer.contains("\x1b[48;2;"));
}

#[test]
fn test_render_braille_output() {
    let geom = RenderGeometry::calculate(10, 5, 20, 20, ScalingMode::Stretch, 2, 4);
    let lut = ColorLut::new(0, 1.0, false, ColorFilter::None);

    let data = vec![255u8; 20 * 20 * 3]; // White frame -> all braille dots active
    let frame = VideoFrame {
        width: 20,
        height: 20,
        pts_secs: 0.0,
        data,
    };

    let mut buffer = String::new();
    render_braille(&frame, &geom, &lut, &mut buffer);

    assert!(!buffer.is_empty());
    assert!(buffer.contains('\u{28FF}')); // All 8 dots on = 0x2800 + 0xFF
}

#[test]
fn test_render_ascii_output() {
    let geom = RenderGeometry::calculate(10, 5, 10, 10, ScalingMode::Stretch, 1, 2);
    let lut = ColorLut::new(0, 1.0, false, ColorFilter::None);

    let data = vec![128u8; 10 * 10 * 3];
    let frame = VideoFrame {
        width: 10,
        height: 10,
        pts_secs: 0.0,
        data,
    };

    let mut buffer = String::new();
    render_ascii(&frame, &geom, &lut, RenderMode::Shades, &mut buffer);
    assert!(!buffer.is_empty());
    assert!(buffer.contains('░') || buffer.contains('▒') || buffer.contains('▓'));
}

#[test]
fn test_clock_pause_resume() {
    let mut clock = Clock::new(10.0);
    clock.start();
    assert!(clock.get_pts() >= 10.0);

    clock.pause();
    let pts_at_pause = clock.get_pts();
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert_eq!(clock.get_pts(), pts_at_pause);

    clock.resume();
    std::thread::sleep(std::time::Duration::from_millis(50));
    assert!(clock.get_pts() > pts_at_pause);
}
