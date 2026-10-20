use std::fmt::Write;
use std::sync::OnceLock;
use crate::render::{ColorLut, RenderGeometry};
use crate::types::VideoFrame;

static SIXEL_PALETTE_HEADER: OnceLock<String> = OnceLock::new();

/// Precomputes the 232-color Sixel palette escape sequence (6x6x6 color cube + 16 grayscale ramp)
fn get_palette_header() -> &'static str {
    SIXEL_PALETTE_HEADER.get_or_init(|| {
        let mut s = String::with_capacity(4096);
        // 6x6x6 color cube (indices 0..215)
        for r_i in 0..6 {
            let r_pct = r_i * 20;
            for g_i in 0..6 {
                let g_pct = g_i * 20;
                for b_i in 0..6 {
                    let b_pct = b_i * 20;
                    let idx = r_i * 36 + g_i * 6 + b_i;
                    let _ = write!(s, "#{};2;{};{};{}", idx, r_pct, g_pct, b_pct);
                }
            }
        }
        // 16-level grayscale ramp (indices 216..231)
        for i in 0..16 {
            let idx = 216 + i;
            let pct = (i * 100) / 15;
            let _ = write!(s, "#{};2;{};{};{}", idx, pct, pct, pct);
        }
        s
    })
}

/// Fast color quantization to 232-color Sixel palette
#[inline(always)]
fn quantize_color(r: u8, g: u8, b: u8) -> u8 {
    let diff_rg = r.abs_diff(g);
    let diff_gb = g.abs_diff(b);
    let diff_rb = r.abs_diff(b);

    if diff_rg < 12 && diff_gb < 12 && diff_rb < 12 {
        // Grayscale ramp (indices 216..231)
        let level = ((r as u32 * 15 + 127) / 255) as u8;
        216 + level.min(15)
    } else {
        // 6x6x6 color cube (indices 0..215)
        let ir = ((r as u32 * 5 + 127) / 255).min(5) as u8;
        let ig = ((g as u32 * 5 + 127) / 255).min(5) as u8;
        let ib = ((b as u32 * 5 + 127) / 255).min(5) as u8;
        ir * 36 + ig * 6 + ib
    }
}

/// Render video frame using the Sixel Graphics Protocol with 256-color palette and RLE compression
pub fn render_sixel(
    frame: &VideoFrame,
    geom: &RenderGeometry,
    lut: &ColorLut,
    buffer: &mut String,
) {
    let w = frame.width as usize;
    let h = frame.height as usize;
    if w == 0 || h == 0 || frame.data.is_empty() {
        return;
    }

    let row = geom.offset_rows + 1;
    let col = geom.offset_cols + 1;

    // Move cursor to top-left of active video region
    let _ = write!(buffer, "\x1b[{};{}H", row, col);

    // DCS Start for Sixel graphics with transparent background mode (7;1;100q)
    let _ = write!(buffer, "\x1bP7;1;100q\"1;1;{};{}", w, h);
    buffer.push_str(get_palette_header());

    // Quantize entire frame to palette indices
    let total_pixels = w * h;
    let mut quantized = vec![0u8; total_pixels];
    let is_ident = lut.is_identity();

    for i in 0..total_pixels {
        let src_idx = i * 3;
        if src_idx + 2 < frame.data.len() {
            let mut r = frame.data[src_idx];
            let mut g = frame.data[src_idx + 1];
            let mut b = frame.data[src_idx + 2];
            if !is_ident {
                let (nr, ng, nb) = lut.transform_rgb(r, g, b);
                r = nr;
                g = ng;
                b = nb;
            }
            quantized[i] = quantize_color(r, g, b);
        }
    }

    let mut sixel_line = vec![0u8; w];
    let mut active = [false; 232];
    let mut band_colors = Vec::with_capacity(32);

    // Process in horizontal bands of 6 vertical pixels
    for by in (0..h).step_by(6) {
        let bh = (h - by).min(6);

        // Find which palette colors are present in this 6-pixel band
        active.fill(false);
        band_colors.clear();

        for dy in 0..bh {
            let row_offset = (by + dy) * w;
            for x in 0..w {
                let c = quantized[row_offset + x] as usize;
                if c < 232 && !active[c] {
                    active[c] = true;
                    band_colors.push(c as u8);
                }
            }
        }

        // Draw each color present in this band
        let color_count = band_colors.len();
        for (color_idx, &c) in band_colors.iter().enumerate() {
            let _ = write!(buffer, "#{}", c);

            let mut last_non_empty: Option<usize> = None;
            for x in 0..w {
                let mut mask = 0u8;
                for dy in 0..bh {
                    if quantized[(by + dy) * w + x] == c {
                        mask |= 1 << dy;
                    }
                }
                let ch = 0x3F + mask;
                sixel_line[x] = ch;
                if mask != 0 {
                    last_non_empty = Some(x);
                }
            }

            // If this color has any pixels in this band, output with RLE
            if let Some(end_x) = last_non_empty {
                let slice = &sixel_line[0..=end_x];
                let mut i = 0;
                while i < slice.len() {
                    let ch = slice[i];
                    let mut run = 1;
                    while i + run < slice.len() && slice[i + run] == ch {
                        run += 1;
                    }
                    if run >= 4 {
                        let _ = write!(buffer, "!{}{}", run, ch as char);
                    } else {
                        for _ in 0..run {
                            buffer.push(ch as char);
                        }
                    }
                    i += run;
                }
            }

            // Carriage return ($) if there are more colors in this band
            if color_idx + 1 < color_count {
                buffer.push('$');
            }
        }

        // Line feed (-) to advance to the next 6-pixel band
        buffer.push('-');
    }

    // String Terminator (ST)
    buffer.push_str("\x1b\\");
}
