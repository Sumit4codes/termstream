use crate::render::{append_bg_rgb, append_fg_rgb, ColorLut, RenderGeometry};
use crate::types::VideoFrame;

/// 16 Unicode quadrant block characters corresponding to 4-bit masks
/// Bit 0: Top-Left (0x1)
/// Bit 1: Top-Right (0x2)
/// Bit 2: Bottom-Left (0x4)
/// Bit 3: Bottom-Right (0x8)
const QUADRANTS: [char; 16] = [
    ' ',        // 0000 (empty)
    '\u{2598}', // 0001: ▘ Top-Left
    '\u{259D}', // 0010: ▝ Top-Right
    '\u{2580}', // 0011: ▀ Upper Half
    '\u{2596}', // 0100: ▖ Bottom-Left
    '\u{258C}', // 0101: ▌ Left Half
    '\u{259E}', // 0110: ▞ Top-Right + Bottom-Left
    '\u{259B}', // 0111: ▛ Top-Left + Top-Right + Bottom-Left
    '\u{2597}', // 1000: ▗ Bottom-Right
    '\u{259A}', // 1001: ▚ Top-Left + Bottom-Right
    '\u{2590}', // 1010: ▐ Right Half
    '\u{259C}', // 1011: ▜ Top-Left + Top-Right + Bottom-Right
    '\u{2584}', // 1100: ▄ Lower Half
    '\u{2599}', // 1101: ▙ Top-Left + Bottom-Left + Bottom-Right
    '\u{259F}', // 1110: ▟ Top-Right + Bottom-Left + Bottom-Right
    '\u{2588}', // 1111: █ Full Block
];

pub fn render_quadrant(
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
        let base_y = vr * 2;

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

        // Active quadrant content (2 subpixels horizontal x 2 subpixels vertical)
        for c in 0..act_c {
            let base_x = c * 2;

            // Fetch 4 subpixels
            let mut rgb = [(0u8, 0u8, 0u8); 4];
            let mut luma = [0u32; 4];

            let coords = [
                (base_x, base_y),         // 0: Top-Left
                (base_x + 1, base_y),     // 1: Top-Right
                (base_x, base_y + 1),     // 2: Bottom-Left
                (base_x + 1, base_y + 1), // 3: Bottom-Right
            ];

            for (i, &(px, py)) in coords.iter().enumerate() {
                if px < frame_w && py < frame_h {
                    let idx = (py * frame_w + px) * 3;
                    if idx + 2 < data.len() {
                        let (r, g, b) = lut.transform_rgb(data[idx], data[idx + 1], data[idx + 2]);
                        rgb[i] = (r, g, b);
                        luma[i] = (r as u32 * 77 + g as u32 * 150 + b as u32 * 29) >> 8;
                    }
                }
            }

            let avg_luma = (luma[0] + luma[1] + luma[2] + luma[3] + 2) / 4;

            let mut mask: usize = 0;
            let mut fg_r = 0u32;
            let mut fg_g = 0u32;
            let mut fg_b = 0u32;
            let mut fg_count = 0u32;

            let mut bg_r = 0u32;
            let mut bg_g = 0u32;
            let mut bg_b = 0u32;
            let mut bg_count = 0u32;

            for i in 0..4 {
                if luma[i] >= avg_luma {
                    mask |= 1 << i;
                    fg_r += rgb[i].0 as u32;
                    fg_g += rgb[i].1 as u32;
                    fg_b += rgb[i].2 as u32;
                    fg_count += 1;
                } else {
                    bg_r += rgb[i].0 as u32;
                    bg_g += rgb[i].1 as u32;
                    bg_b += rgb[i].2 as u32;
                    bg_count += 1;
                }
            }

            if fg_count == 0 {
                let col = (
                    (bg_r / bg_count.max(1)) as u8,
                    (bg_g / bg_count.max(1)) as u8,
                    (bg_b / bg_count.max(1)) as u8,
                );
                if last_bg != Some(col) {
                    append_bg_rgb(buffer, col.0, col.1, col.2);
                    last_bg = Some(col);
                }
                buffer.push(' ');
            } else if bg_count == 0 {
                let col = (
                    (fg_r / fg_count) as u8,
                    (fg_g / fg_count) as u8,
                    (fg_b / fg_count) as u8,
                );
                if last_fg != Some(col) {
                    append_fg_rgb(buffer, col.0, col.1, col.2);
                    last_fg = Some(col);
                }
                if last_bg.is_some() {
                    buffer.push_str("\x1b[49m");
                    last_bg = None;
                }
                buffer.push('\u{2588}');
            } else {
                let fg_col = (
                    (fg_r / fg_count) as u8,
                    (fg_g / fg_count) as u8,
                    (fg_b / fg_count) as u8,
                );
                let bg_col = (
                    (bg_r / bg_count) as u8,
                    (bg_g / bg_count) as u8,
                    (bg_b / bg_count) as u8,
                );

                if last_fg != Some(fg_col) {
                    append_fg_rgb(buffer, fg_col.0, fg_col.1, fg_col.2);
                    last_fg = Some(fg_col);
                }
                if last_bg != Some(bg_col) {
                    append_bg_rgb(buffer, bg_col.0, bg_col.1, bg_col.2);
                    last_bg = Some(bg_col);
                }
                buffer.push(QUADRANTS[mask]);
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
