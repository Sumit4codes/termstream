use std::fmt::Write;
use crate::render::{ColorLut, RenderGeometry};
use crate::types::{RenderMode, VideoFrame};

const ASCII_RAMP: &[u8] = b" .'`^\",:;Il!i><~+_-?][}{1)(|\\/tfjrxnuvczXYUJCLQ0OZmwqpdbkhao*#MW&8%B@$";
const ASCII_SHORT_RAMP: &[u8] = b" .:-=+*#%@";
const SHADE_CHARS: &[char] = &[' ', '░', '▒', '▓', '█'];

pub fn render_ascii(
    frame: &VideoFrame,
    geom: &RenderGeometry,
    lut: &ColorLut,
    mode: RenderMode,
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
        let y0 = vr * 2;
        let y1 = y0 + 1;

        if off_c > 0 {
            if last_fg.is_some() {
                buffer.push_str("\x1b[0m");
                last_fg = None;
            }
            for _ in 0..off_c {
                buffer.push(' ');
            }
        }

        for c in 0..act_c {
            let (r0, g0, b0) = if y0 < frame_h && c < frame_w {
                let idx = (y0 * frame_w + c) * 3;
                if idx + 2 < data.len() {
                    (data[idx], data[idx + 1], data[idx + 2])
                } else {
                    (0, 0, 0)
                }
            } else {
                (0, 0, 0)
            };

            let (r1, g1, b1) = if y1 < frame_h && c < frame_w {
                let idx = (y1 * frame_w + c) * 3;
                if idx + 2 < data.len() {
                    (data[idx], data[idx + 1], data[idx + 2])
                } else {
                    (0, 0, 0)
                }
            } else {
                (0, 0, 0)
            };

            let avg_raw_r = ((r0 as u32 + r1 as u32) / 2) as u8;
            let avg_raw_g = ((g0 as u32 + g1 as u32) / 2) as u8;
            let avg_raw_b = ((b0 as u32 + b1 as u32) / 2) as u8;

            let (adj_r, adj_g, adj_b) = lut.transform_rgb(avg_raw_r, avg_raw_g, avg_raw_b);
            let y = ((adj_r as u32 * 77 + adj_g as u32 * 150 + adj_b as u32 * 29) >> 8) as u8;

            match mode {
                RenderMode::Shades => {
                    let idx = (y as usize * (SHADE_CHARS.len() - 1)) / 255;
                    let ch = SHADE_CHARS[idx];

                    if last_fg != Some((adj_r, adj_g, adj_b)) {
                        let _ = write!(buffer, "\x1b[38;2;{};{};{}m", adj_r, adj_g, adj_b);
                        last_fg = Some((adj_r, adj_g, adj_b));
                    }
                    buffer.push(ch);
                }
                RenderMode::Ascii => {
                    let idx = (y as usize * (ASCII_SHORT_RAMP.len() - 1)) / 255;
                    let ch = ASCII_SHORT_RAMP[idx] as char;

                    if last_fg != Some((adj_r, adj_g, adj_b)) {
                        let _ = write!(buffer, "\x1b[38;2;{};{};{}m", adj_r, adj_g, adj_b);
                        last_fg = Some((adj_r, adj_g, adj_b));
                    }
                    buffer.push(ch);
                }
                RenderMode::Mono => {
                    let idx = (y as usize * (ASCII_RAMP.len() - 1)) / 255;
                    let ch = ASCII_RAMP[idx] as char;

                    // Classic Green CRT Phosphor
                    let mono_fg = (
                        (adj_r as f32 * 0.2) as u8,
                        (adj_g as f32 * 0.95).max(40.0) as u8,
                        (adj_b as f32 * 0.2) as u8,
                    );

                    if last_fg != Some(mono_fg) {
                        let _ = write!(buffer, "\x1b[38;2;{};{};{}m", mono_fg.0, mono_fg.1, mono_fg.2);
                        last_fg = Some(mono_fg);
                    }
                    buffer.push(ch);
                }
                _ => {}
            }
        }

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
