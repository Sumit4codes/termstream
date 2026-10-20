use std::io::Read;
use std::process::{Command, Stdio};
use std::thread;
use anyhow::{Context, Result};
use crossterm::terminal::size as terminal_size;

use crate::generator::{ProceduralGenerator, TestPattern};
use crate::render::ascii::render_ascii;
use crate::render::braille::render_braille;
use crate::render::half_block::render_half_block;
use crate::render::quadrant::render_quadrant;
use crate::render::{ColorLut, RenderGeometry};
use crate::types::{ColorFilter, MediaMetadata, RenderMode, VideoFrame};
use crate::ui::format_time;

/// Safely truncate a UTF-8 string by character count (never panics on multi-byte boundaries)
pub fn safe_truncate(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count > max_chars {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{}…", truncated)
    } else {
        s.to_string()
    }
}

/// Extract a single frame from video at timestamp using FFmpeg
pub fn extract_frame(
    path: &str,
    target_w: u32,
    target_h: u32,
    timestamp_secs: f64,
) -> Result<VideoFrame> {
    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error"]);
    if timestamp_secs > 0.0 {
        cmd.args(["-ss", &format!("{:.3}", timestamp_secs)]);
    }
    cmd.args([
        "-i", path,
        "-an",
        "-sn",
        "-vframes", "1",
        "-f", "rawvideo",
        "-pix_fmt", "rgb24",
        "-s", &format!("{}x{}", target_w, target_h),
        "pipe:1",
    ]);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.stdin(Stdio::null());

    let mut child = cmd.spawn().context("Failed to spawn ffmpeg for frame extraction")?;
    let mut stdout = child.stdout.take().context("Failed to take stdout")?;
    let frame_size = (target_w * target_h * 3) as usize;
    let mut data = vec![0u8; frame_size];
    stdout.read_exact(&mut data).context("Failed to read frame data from ffmpeg")?;
    let _ = child.wait();

    Ok(VideoFrame {
        width: target_w,
        height: target_h,
        pts_secs: timestamp_secs,
        data,
    })
}

/// Render a 3x3 keyframe storyboard contact sheet to terminal stdout
pub fn print_storyboard(
    video_path: Option<&str>,
    pattern: Option<TestPattern>,
    meta: &MediaMetadata,
    mode: RenderMode,
    filter: ColorFilter,
) -> Result<()> {
    let (term_cols, _) = terminal_size().unwrap_or((80, 24));
    let lut = ColorLut::new(0, 1.0, false, filter);

    let raw_title = meta.title.as_deref().unwrap_or("Media Stream");
    let display_title = safe_truncate(raw_title, 25);
    let duration = if meta.duration_secs > 0.0 { meta.duration_secs } else { 30.0 };

    println!("\x1b[1;38;2;120;180;255m┌─────────────────────────── TermStream Storyboard ───────────────────────────┐\x1b[0m");
    println!(
        "\x1b[38;2;200;210;230m│ \x1b[1m{:<25}\x1b[0m \x1b[38;2;150;160;180mDuration: \x1b[38;2;255;255;255m{}\x1b[38;2;150;160;180m  Res: \x1b[38;2;255;255;255m{}x{}\x1b[38;2;150;160;180m  FPS: \x1b[38;2;255;255;255m{:.1}\x1b[38;2;200;210;230m │\x1b[0m",
        display_title,
        format_time(meta.duration_secs),
        meta.width,
        meta.height,
        meta.fps
    );
    println!("\x1b[1;38;2;120;180;255m└─────────────────────────────────────────────────────────────────────────────┘\x1b[0m\n");

    const COLS: usize = 3;
    const ROWS: usize = 3;
    const GAP_X: usize = 2;

    // Calculate width of each thumbnail in character cells
    let available_width = (term_cols as usize).saturating_sub(GAP_X * (COLS - 1)).max(30);
    let cell_w = (available_width / COLS).clamp(16, 40) as u16;

    // Calculate height of each thumbnail in character cells
    let video_ar = (meta.width as f64 / meta.height as f64).clamp(0.5, 3.0);
    let cell_h = ((cell_w as f64 * 0.5) / video_ar).round().clamp(6.0, 16.0) as u16;

    let (px_c, px_r) = match mode {
        RenderMode::Quadrant => (2, 2),
        RenderMode::Braille => (2, 4),
        _ => (1, 2),
    };

    let geom = RenderGeometry {
        canvas_cols: cell_w,
        canvas_rows: cell_h,
        pixel_width: cell_w as u32 * px_c,
        pixel_height: cell_h as u32 * px_r,
        offset_cols: 0,
        offset_rows: 0,
        active_cols: cell_w,
        active_rows: cell_h,
    };

    let total_frames = COLS * ROWS;

    // Extract all 9 keyframes concurrently in parallel (dramatically speeds up web/YouTube streams)
    let all_frames: Vec<Option<VideoFrame>> = thread::scope(|s| {
        let handles: Vec<_> = (0..total_frames)
            .map(|idx| {
                let time_secs = duration * ((idx + 1) as f64 / (total_frames + 1) as f64);
                s.spawn(move || {
                    if let Some(pat) = pattern {
                        let mut data = vec![0u8; (geom.pixel_width * geom.pixel_height * 3) as usize];
                        ProceduralGenerator::render_frame(pat, geom.pixel_width, geom.pixel_height, time_secs, &mut data);
                        Some(VideoFrame {
                            width: geom.pixel_width,
                            height: geom.pixel_height,
                            pts_secs: time_secs,
                            data,
                        })
                    } else if let Some(path) = video_path {
                        extract_frame(path, geom.pixel_width, geom.pixel_height, time_secs).ok()
                    } else {
                        None
                    }
                })
            })
            .collect();

        handles.into_iter().map(|h| h.join().unwrap_or(None)).collect()
    });

    for grid_row in 0..ROWS {
        // 1. Print timestamps header for this row
        for grid_col in 0..COLS {
            let idx = grid_row * COLS + grid_col;
            let time_secs = duration * ((idx + 1) as f64 / (total_frames + 1) as f64);
            let time_label = format!("[ {} ]", format_time(time_secs));
            let pad = (cell_w as usize).saturating_sub(time_label.len());
            let left_pad = pad / 2;
            let right_pad = pad.saturating_sub(left_pad);

            print!(
                "\x1b[38;2;140;180;240m{:left$}{}{:right$}\x1b[0m",
                "",
                time_label,
                "",
                left = left_pad,
                right = right_pad
            );
            if grid_col + 1 < COLS {
                print!("{:gap$}", "", gap = GAP_X);
            }
        }
        println!();

        // 2. Collect frames for this row
        let row_frames: Vec<&Option<VideoFrame>> = (0..COLS)
            .map(|grid_col| &all_frames[grid_row * COLS + grid_col])
            .collect();

        // 3. Render lines side-by-side
        let mut cell_lines: Vec<Vec<String>> = Vec::new();
        for maybe_frame in row_frames {
            let mut lines = Vec::new();
            if let Some(frame) = maybe_frame {
                let mut buf = String::new();
                match mode {
                    RenderMode::Quadrant => render_quadrant(frame, &geom, &lut, &mut buf),
                    RenderMode::Braille => render_braille(frame, &geom, &lut, &mut buf),
                    RenderMode::Ascii | RenderMode::Shades | RenderMode::Mono => {
                        render_ascii(frame, &geom, &lut, mode, &mut buf)
                    }
                    _ => render_half_block(frame, &geom, &lut, &mut buf),
                }
                for line in buf.lines() {
                    lines.push(line.to_string());
                }
            }
            cell_lines.push(lines);
        }

        for line_idx in 0..cell_h as usize {
            for grid_col in 0..COLS {
                if let Some(lines) = cell_lines.get(grid_col) {
                    if let Some(line) = lines.get(line_idx) {
                        print!("{}\x1b[0m", line);
                    } else {
                        print!("{:width$}", "", width = cell_w as usize);
                    }
                } else {
                    print!("{:width$}", "", width = cell_w as usize);
                }
                if grid_col + 1 < COLS {
                    print!("{:gap$}", "", gap = GAP_X);
                }
            }
            println!();
        }
        println!();
    }

    Ok(())
}
