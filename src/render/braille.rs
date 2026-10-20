use std::fmt::Write;
use crate::render::{ColorLut, RenderGeometry};
use crate::types::VideoFrame;

pub fn render_braille(
    frame: &VideoFrame,
    geom: &RenderGeometry,
    lut: &ColorLut,
    buffer: &mut String,
) {
    let frame_w = frame.width as usize;
    let frame_h = frame.height as usize;
    let data = &frame.data;

    let mut last_fg: Option<(u8, u8, u8)> = None;
    let mut last_bg: Option<(u8, u8, u8)> = None;

    let total_rows = geom.canvas_rows as usize;
    let total_cols = geom.canvas_cols as usize;
    let off_r = geom.offset_rows as usize;
    let off_c = geom.offset_cols as usize;
    let act_r = geom.active_rows as usize;
    let act_c = geom.active_cols as usize;

    const DOTS: [(usize, usize, u32); 8] = [
        (0, 0, 0x01),
        (0, 1, 0x02),
        (0, 2, 0x04),
        (1, 0, 0x08),
        (1, 1, 0x10),
        (1, 2, 0x20),
        (0, 3, 0x40),
        (1, 3, 0x80),
    ];

    for r in 0..total_rows {
        // Vertical letterbox bars
        if r < off_r || r >= off_r + act_r {
            if last_fg.is_some() || last_bg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
                last_bg = None;
            }
            for _ in 0..total_cols {
                buffer.push(' ');
            }
            if r + 1 < total_rows {
                buffer.push_str("\r\n");
            }
            continue;
        }

        let vr = r - off_r;
        let base_y = vr * 4;

        // Left letterbox bar
        if off_c > 0 {
            if last_fg.is_some() || last_bg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
                last_bg = None;
            }
            for _ in 0..off_c {
                buffer.push(' ');
            }
        }

        // Active braille content (2 subpixels horizontal x 4 subpixels vertical)
        for c in 0..act_c {
            let base_x = c * 2;

            let mut rgb = [(0u8, 0u8, 0u8); 8];
            let mut luma = [0u32; 8];
            let mut min_y = 255u32;
            let mut max_y = 0u32;
            let mut sum_y = 0u32;

            for (i, &(dx, dy, _)) in DOTS.iter().enumerate() {
                let px = base_x + dx;
                let py = base_y + dy;

                if px < frame_w && py < frame_h {
                    let idx = (py * frame_w + px) * 3;
                    if idx + 2 < data.len() {
                        let (r, g, b) = lut.transform_rgb(data[idx], data[idx + 1], data[idx + 2]);
                        rgb[i] = (r, g, b);
                        let y = (r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8;
                        luma[i] = y;
                        if y < min_y { min_y = y; }
                        if y > max_y { max_y = y; }
                        sum_y += y;
                    }
                }
            }

            let contrast_range = max_y.saturating_sub(min_y);
            let mut pattern: u32 = 0;
            let mut total_r_fg = 0u32;
            let mut total_g_fg = 0u32;
            let mut total_b_fg = 0u32;
            let mut count_fg = 0u32;

            let mut total_r_bg = 0u32;
            let mut total_g_bg = 0u32;
            let mut total_b_bg = 0u32;
            let mut count_bg = 0u32;

            if contrast_range < 16 {
                // Smooth region: classify based on absolute brightness
                let is_bright = (sum_y / 8) >= 128;
                for (i, &(_, _, bit)) in DOTS.iter().enumerate() {
                    if is_bright {
                        pattern |= bit;
                        total_r_fg += rgb[i].0 as u32;
                        total_g_fg += rgb[i].1 as u32;
                        total_b_fg += rgb[i].2 as u32;
                        count_fg += 1;
                    } else {
                        total_r_bg += rgb[i].0 as u32;
                        total_g_bg += rgb[i].1 as u32;
                        total_b_bg += rgb[i].2 as u32;
                        count_bg += 1;
                    }
                }
            } else {
                // High-detail / edge region: adaptive thresholding for maximum sharpness
                let mid_y = (min_y + max_y) / 2;
                for (i, &(_, _, bit)) in DOTS.iter().enumerate() {
                    if luma[i] >= mid_y {
                        pattern |= bit;
                        total_r_fg += rgb[i].0 as u32;
                        total_g_fg += rgb[i].1 as u32;
                        total_b_fg += rgb[i].2 as u32;
                        count_fg += 1;
                    } else {
                        total_r_bg += rgb[i].0 as u32;
                        total_g_bg += rgb[i].1 as u32;
                        total_b_bg += rgb[i].2 as u32;
                        count_bg += 1;
                    }
                }
            }

            if count_fg == 0 {
                let bg_r = (total_r_bg / count_bg.max(1)) as u8;
                let bg_g = (total_g_bg / count_bg.max(1)) as u8;
                let bg_b = (total_b_bg / count_bg.max(1)) as u8;

                if last_bg != Some((bg_r, bg_g, bg_b)) {
                    let _ = write!(buffer, "\x1b[48;2;{};{};{}m", bg_r, bg_g, bg_b);
                    last_bg = Some((bg_r, bg_g, bg_b));
                }
                buffer.push(' ');
            } else if count_bg == 0 {
                let fg_r = (total_r_fg / count_fg) as u8;
                let fg_g = (total_g_fg / count_fg) as u8;
                let fg_b = (total_b_fg / count_fg) as u8;

                if last_fg != Some((fg_r, fg_g, fg_b)) {
                    let _ = write!(buffer, "\x1b[38;2;{};{};{}m", fg_r, fg_g, fg_b);
                    last_fg = Some((fg_r, fg_g, fg_b));
                }
                if last_bg.is_some() {
                    buffer.push_str("\x1b[49m");
                    last_bg = None;
                }
                buffer.push('\u{28FF}');
            } else {
                let fg_r = (total_r_fg / count_fg) as u8;
                let fg_g = (total_g_fg / count_fg) as u8;
                let fg_b = (total_b_fg / count_fg) as u8;

                let bg_r = (total_r_bg / count_bg) as u8;
                let bg_g = (total_g_bg / count_bg) as u8;
                let bg_b = (total_b_bg / count_bg) as u8;

                if last_fg != Some((fg_r, fg_g, fg_b)) {
                    let _ = write!(buffer, "\x1b[38;2;{};{};{}m", fg_r, fg_g, fg_b);
                    last_fg = Some((fg_r, fg_g, fg_b));
                }
                if last_bg != Some((bg_r, bg_g, bg_b)) {
                    let _ = write!(buffer, "\x1b[48;2;{};{};{}m", bg_r, bg_g, bg_b);
                    last_bg = Some((bg_r, bg_g, bg_b));
                }

                let ch = std::char::from_u32(0x2800 + pattern).unwrap_or(' ');
                buffer.push(ch);
            }
        }

        // Right letterbox bar
        let right_pad = total_cols.saturating_sub(off_c + act_c);
        if right_pad > 0 {
            if last_fg.is_some() || last_bg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
                last_bg = None;
            }
            for _ in 0..right_pad {
                buffer.push(' ');
            }
        }

        if r + 1 < total_rows {
            buffer.push_str("\r\n");
        }
    }

    if last_fg.is_some() || last_bg.is_some() {
        buffer.push_str("\x1b[0m");
    }
}
