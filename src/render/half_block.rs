use crate::render::{append_bg_rgb, append_fg_rgb, ColorLut, RenderGeometry};
use crate::types::VideoFrame;

pub fn render_half_block(
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
        let top_y = vr * 2;
        let bot_y = top_y + 1;

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

        // Active video content
        let top_row_offset = top_y * frame_w * 3;
        let bot_row_offset = bot_y * frame_w * 3;

        for c in 0..act_c {
            // Check boundary conditions safely
            let (top_r, top_g, top_b) = if top_y < frame_h && c < frame_w {
                let idx = top_row_offset + c * 3;
                if idx + 2 < data.len() {
                    lut.transform_rgb(data[idx], data[idx + 1], data[idx + 2])
                } else {
                    (0, 0, 0)
                }
            } else {
                (0, 0, 0)
            };

            let (bot_r, bot_g, bot_b) = if bot_y < frame_h && c < frame_w {
                let idx = bot_row_offset + c * 3;
                if idx + 2 < data.len() {
                    lut.transform_rgb(data[idx], data[idx + 1], data[idx + 2])
                } else {
                    (0, 0, 0)
                }
            } else {
                (0, 0, 0)
            };

            // Optimize ANSI escape sequences
            if last_fg != Some((top_r, top_g, top_b)) {
                append_fg_rgb(buffer, top_r, top_g, top_b);
                last_fg = Some((top_r, top_g, top_b));
            }
            if last_bg != Some((bot_r, bot_g, bot_b)) {
                append_bg_rgb(buffer, bot_r, bot_g, bot_b);
                last_bg = Some((bot_r, bot_g, bot_b));
            }

            buffer.push('▀');
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
