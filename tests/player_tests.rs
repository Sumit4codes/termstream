use termstream::clock::Clock;
use termstream::render::ascii::render_ascii;
use termstream::render::braille::render_braille;
use termstream::render::half_block::render_half_block;
use termstream::render::quadrant::render_quadrant;
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
fn test_render_quadrant_output() {
    let geom = RenderGeometry::calculate(10, 5, 20, 10, ScalingMode::Stretch, 2, 2);
    let lut = ColorLut::new(0, 1.0, false, ColorFilter::None);

    let mut data = vec![0u8; 20 * 10 * 3];
    // Fill top-left subpixel of each cell with bright green (0, 255, 0)
    data[0 + 1] = 255;

    let frame = VideoFrame {
        width: 20,
        height: 10,
        pts_secs: 0.0,
        data,
    };

    let mut buffer = String::new();
    render_quadrant(&frame, &geom, &lut, &mut buffer);

    assert!(!buffer.is_empty());
    assert!(buffer.contains("\x1b[38;2;"));
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

#[test]
fn test_pcm_source_chunked_streaming() {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use termstream::audio::PcmSource;

    let (tx, rx) = crossbeam_channel::bounded::<Vec<i16>>(4);
    let samples_played = Arc::new(AtomicU64::new(0));
    let mut source = PcmSource::new(rx, 44100, 2, Arc::clone(&samples_played));

    // Send chunk of 4 samples
    tx.send(vec![100, 200, -300, -400]).unwrap();

    assert_eq!(source.next(), Some(100));
    assert_eq!(source.next(), Some(200));
    assert_eq!(source.next(), Some(-300));
    assert_eq!(source.next(), Some(-400));
    assert_eq!(samples_played.load(Ordering::Relaxed), 4);

    // When empty, should yield silence without blocking
    assert_eq!(source.next(), Some(0));
    assert_eq!(samples_played.load(Ordering::Relaxed), 5);

    // Drop sender -> Disconnected -> iterator should return None
    drop(tx);
    assert_eq!(source.next(), None);
}

#[test]
fn test_audio_player_volume_and_preamp() {
    use termstream::audio::AudioPlayer;

    // Disabled audio player (no sound hardware needed in unit tests)
    let mut player = AudioPlayer::new("dummy.mp4".into(), false);

    player.set_volume(1.0);
    assert_eq!(player.get_volume(), 1.0);
    assert!((player.effective_volume() - 1.35).abs() < 0.001);

    // Clamps to max 3.0 (300%)
    player.set_volume(4.5);
    assert_eq!(player.get_volume(), 3.0);
    assert!((player.effective_volume() - 3.0 * AudioPlayer::PREAMP_GAIN).abs() < 0.001);

    // Clamps to min 0.0
    player.set_volume(-1.0);
    assert_eq!(player.get_volume(), 0.0);

    // Mute toggle
    assert!(!player.is_muted());
    assert!(player.toggle_mute());
    assert!(player.is_muted());
    assert!(!player.toggle_mute());
    assert!(!player.is_muted());
}

#[test]
fn test_ytdl_url_parsing_and_binary_detection() {
    use termstream::ytdl::YtDlp;

    // Web URL detection
    assert!(YtDlp::is_web_url("https://www.youtube.com/watch?v=dQw4w9WgXcQ"));
    assert!(YtDlp::is_web_url("http://vimeo.com/123456"));
    assert!(YtDlp::is_web_url("yt:dQw4w9WgXcQ"));
    assert!(YtDlp::is_web_url("ytdl:https://example.com/video.mp4"));
    assert!(YtDlp::is_web_url("youtube:dQw4w9WgXcQ"));
    assert!(!YtDlp::is_web_url("/path/to/local/movie.mp4"));
    assert!(!YtDlp::is_web_url("test:plasma"));

    // URL normalization
    assert_eq!(
        YtDlp::normalize_url("yt:dQw4w9WgXcQ"),
        "https://www.youtube.com/watch?v=dQw4w9WgXcQ"
    );
    assert_eq!(
        YtDlp::normalize_url("youtube:FtutLA63Cp8"),
        "https://www.youtube.com/watch?v=FtutLA63Cp8"
    );
    assert_eq!(
        YtDlp::normalize_url("ytdl:https://vimeo.com/123"),
        "https://vimeo.com/123"
    );
    assert_eq!(
        YtDlp::normalize_url("https://youtu.be/dQw4w9WgXcQ"),
        "https://youtu.be/dQw4w9WgXcQ"
    );

    // Finding binary should succeed if yt-dlp is present
    if let Ok(bin) = YtDlp::find_binary() {
        assert!(bin.to_string_lossy().contains("yt-dlp"));
    }
}

#[test]
fn test_storyboard_procedural_generation() {
    use termstream::generator::{ProceduralGenerator, TestPattern};
    use termstream::storyboard::print_storyboard;
    use termstream::types::{ColorFilter, RenderMode};

    let meta = ProceduralGenerator::metadata(TestPattern::Bars);
    let result = print_storyboard(
        None,
        Some(TestPattern::Bars),
        &meta,
        RenderMode::HalfBlock,
        ColorFilter::None,
    );
    assert!(result.is_ok());
}

#[test]
fn test_safe_truncate_multibyte_utf8() {
    use termstream::storyboard::safe_truncate;

    // Multi-byte CJK string that panicked previously
    let cjk = "【東方】Bad Apple!! ＰＶ【影絵】";
    let truncated_cjk = safe_truncate(cjk, 10);
    assert!(truncated_cjk.ends_with('…'));
    assert_eq!(truncated_cjk.chars().count(), 10);

    // Emoji string
    let emoji = "🎬📺🚀 TermStream is blazing fast and super awesome!";
    let truncated_emoji = safe_truncate(emoji, 12);
    assert!(truncated_emoji.ends_with('…'));
    assert_eq!(truncated_emoji.chars().count(), 12);

    // Short string (no truncation needed)
    let short = "Short Title";
    assert_eq!(safe_truncate(short, 25), "Short Title");
}

#[test]
fn test_render_kitty_output() {
    use termstream::render::kitty::render_kitty;
    use termstream::render::{ColorLut, RenderGeometry};
    use termstream::types::{ColorFilter, VideoFrame};

    let w = 16u32;
    let h = 16u32;
    let mut data = vec![0u8; (w * h * 3) as usize];
    // Fill with sample gradient
    for i in 0..(w * h) as usize {
        data[i * 3] = (i % 256) as u8;
        data[i * 3 + 1] = ((i * 2) % 256) as u8;
        data[i * 3 + 2] = 255;
    }

    let frame = VideoFrame {
        width: w,
        height: h,
        pts_secs: 0.0,
        data,
    };

    let geom = RenderGeometry {
        canvas_cols: 16,
        canvas_rows: 16,
        pixel_width: w,
        pixel_height: h,
        offset_cols: 2,
        offset_rows: 1,
        active_cols: 16,
        active_rows: 16,
    };

    let lut = ColorLut::new(0, 1.0, false, ColorFilter::None);
    let mut buffer = String::new();
    render_kitty(&frame, &geom, &lut, &mut buffer);

    // Verify deletion of previous placement
    assert!(buffer.contains("\x1b_Ga=d,d=i,i=1\x1b\\"));
    // Verify cursor positioning at offset (row 2, col 3)
    assert!(buffer.contains("\x1b[2;3H"));
    // Verify transmit and display command
    assert!(buffer.contains("a=T,f=24,s=16,v=16,c=16,r=16,z=-1,q=2,i=1"));
    // Verify clean APC termination
    assert!(buffer.ends_with("\x1b\\"));
}

#[test]
fn test_render_sixel_output() {
    use termstream::render::sixel::render_sixel;
    use termstream::render::{ColorLut, RenderGeometry};
    use termstream::types::{ColorFilter, VideoFrame};

    let w = 12u32;
    let h = 12u32;
    let mut data = vec![0u8; (w * h * 3) as usize];
    // Flat blue test frame
    for i in 0..(w * h) as usize {
        data[i * 3] = 0;
        data[i * 3 + 1] = 0;
        data[i * 3 + 2] = 255;
    }

    let frame = VideoFrame {
        width: w,
        height: h,
        pts_secs: 0.0,
        data,
    };

    let geom = RenderGeometry {
        canvas_cols: 12,
        canvas_rows: 12,
        pixel_width: w,
        pixel_height: h,
        offset_cols: 0,
        offset_rows: 0,
        active_cols: 12,
        active_rows: 12,
    };

    let lut = ColorLut::new(0, 1.0, false, ColorFilter::None);
    let mut buffer = String::new();
    render_sixel(&frame, &geom, &lut, &mut buffer);

    // Verify Sixel DCS header and raster dimensions
    assert!(buffer.contains("\x1bP7;1;100q\"1;1;12;12"));
    // Verify palette definition is embedded
    assert!(buffer.contains("#0;2;0;0;0"));
    // Verify RLE compression is used (runs of >= 4 repeated characters)
    assert!(buffer.contains("!"));
    // Verify Sixel string terminator
    assert!(buffer.ends_with("\x1b\\"));
}

#[test]
fn test_render_modes_pixel_cycle() {
    use termstream::types::RenderMode;

    let start = RenderMode::HalfBlock;
    let m1 = start.next();
    assert_eq!(m1, RenderMode::Quadrant);
    let m2 = m1.next();
    assert_eq!(m2, RenderMode::Braille);
    let m3 = m2.next();
    assert_eq!(m3, RenderMode::Shades);
    let m4 = m3.next();
    assert_eq!(m4, RenderMode::Ascii);
    let m5 = m4.next();
    assert_eq!(m5, RenderMode::Mono);
    let m6 = m5.next();
    assert_eq!(m6, RenderMode::Kitty);
    let m7 = m6.next();
    assert_eq!(m7, RenderMode::Sixel);
    let loop_back = m7.next();
    assert_eq!(loop_back, RenderMode::HalfBlock);

    assert_eq!(RenderMode::Kitty.subpixels(), (8, 16));
    assert_eq!(RenderMode::Sixel.subpixels(), (6, 12));
    assert!(RenderMode::Kitty.name().contains("Kitty Pixel Graphics"));
    assert!(RenderMode::Sixel.name().contains("Sixel Pixel Graphics"));
}
