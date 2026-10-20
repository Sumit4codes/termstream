use std::fmt::Write;
use crate::render::{ColorLut, RenderGeometry};
use crate::types::VideoFrame;

const BAYER_2X4: [[u8; 2]; 4] = [
    [ 20, 140],
    [200,  80],
    [ 60, 180],
    [220, 100],
];

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

    let total_rows = geom.canvas_rows as usize;
    let total_cols = geom.canvas_cols as usize;
    let off_r = geom.offset_rows as usize;
    let off_c = geom.offset_cols as usize;
    let act_r = geom.active_rows as usize;
    let act_c = geom.active_cols as usize;

    for r in 0..total_rows {
        // Vertical letterbox bars
        if r < off_r || r >= off_r + act_r {
            if last_fg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
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
            if last_fg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
            }
            for _ in 0..off_c {
                buffer.push(' ');
            }
        }

        // Active braille content
        for c in 0..act_c {
            let base_x = c * 2;
            let mut pattern: u32 = 0;
            let mut total_r: u32 = 0;
            let mut total_g: u32 = 0;
            let mut total_b: u32 = 0;
            let mut active_count: u32 = 0;

            // Coordinates in the 2x4 matrix:
            // (dx, dy, bit)
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

            for (dx, dy, bit) in DOTS {
                let px = base_x + dx;
                let py = base_y + dy;

                if px < frame_w && py < frame_h {
                    let idx = (py * frame_w + px) * 3;
                    if idx + 2 < data.len() {
                        let (r, g, b) = lut.transform_rgb(data[idx], data[idx + 1], data[idx + 2]);
                        let y = ((r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8) as u8;
                        let threshold = BAYER_2X4[dy][dx];

                        if y > threshold {
                            pattern |= bit;
                            total_r += r as u32;
                            total_g += g as u32;
                            total_b += b as u32;
                            active_count += 1;
                        }
                    }
                }
            }

            if pattern == 0 {
                // Empty braille space
                buffer.push(' ');
            } else {
                let avg_r = (total_r / active_count) as u8;
                let avg_g = (total_g / active_count) as u8;
                let avg_b = (total_b / active_count) as u8;

                if last_fg != Some((avg_r, avg_g, avg_b)) {
                    let _ = write!(buffer, "\x1b[38;2;{};{};{}m", avg_r, avg_g, avg_b);
                    last_fg = Some((avg_r, avg_g, avg_b));
                }

                let ch = std::char::from_u32(0x2800 + pattern).unwrap_or(' ');
                buffer.push(ch);
            }
        }

        // Right letterbox bar
        let right_pad = total_cols.saturating_sub(off_c + act_c);
        if right_pad > 0 {
            if last_fg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
            }
            for _ in 0..right_pad {
                buffer.push(' ');
            }
        }

        if r + 1 < total_rows {
            buffer.push_str("\r\n");
        }
    }

    if last_fg.is_some() {
        buffer.push_str("\x1b[0m");
    }
}
