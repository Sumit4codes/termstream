pub mod half_block;
pub mod braille;
pub mod ascii;

use crate::types::{ColorFilter, ScalingMode};

#[derive(Debug, Clone)]
pub struct ColorLut {
    pub map_r: [u8; 256],
    pub map_g: [u8; 256],
    pub map_b: [u8; 256],
    pub filter: ColorFilter,
}

impl ColorLut {
    pub fn new(brightness: i32, contrast: f32, invert: bool, filter: ColorFilter) -> Self {
        let mut map_r = [0u8; 256];
        let mut map_g = [0u8; 256];
        let mut map_b = [0u8; 256];

        for i in 0..256 {
            let mut val = i as f32;

            // Invert
            if invert {
                val = 255.0 - val;
            }

            // Contrast
            val = (val - 128.0) * contrast + 128.0;

            // Brightness
            val += brightness as f32;

            let clamped = val.clamp(0.0, 255.0) as u8;
            map_r[i] = clamped;
            map_g[i] = clamped;
            map_b[i] = clamped;
        }

        Self {
            map_r,
            map_g,
            map_b,
            filter,
        }
    }

    #[inline(always)]
    pub fn transform_rgb(&self, r: u8, g: u8, b: u8) -> (u8, u8, u8) {
        let r_adj = self.map_r[r as usize];
        let g_adj = self.map_g[g as usize];
        let b_adj = self.map_b[b as usize];

        match self.filter {
            ColorFilter::None => (r_adj, g_adj, b_adj),
            ColorFilter::Grayscale => {
                let y = ((r_adj as u32 * 77 + g_adj as u32 * 150 + b_adj as u32 * 29) >> 8) as u8;
                (y, y, y)
            }
            ColorFilter::MatrixGreen => {
                let y = ((r_adj as u32 * 77 + g_adj as u32 * 150 + b_adj as u32 * 29) >> 8) as u8;
                ((y as f32 * 0.15) as u8, y, (y as f32 * 0.25) as u8)
            }
            ColorFilter::AmberCrt => {
                let y = ((r_adj as u32 * 77 + g_adj as u32 * 150 + b_adj as u32 * 29) >> 8) as u8;
                (y, (y as f32 * 0.72) as u8, (y as f32 * 0.05) as u8)
            }
            ColorFilter::CyberCyan => {
                let y = ((r_adj as u32 * 77 + g_adj as u32 * 150 + b_adj as u32 * 29) >> 8) as u8;
                ((y as f32 * 0.1) as u8, (y as f32 * 0.9) as u8, y)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RenderGeometry {
    pub canvas_cols: u16,
    pub canvas_rows: u16,
    pub pixel_width: u32,
    pub pixel_height: u32,
    pub offset_cols: u16,
    pub offset_rows: u16,
    pub active_cols: u16,
    pub active_rows: u16,
}

impl RenderGeometry {
    pub fn calculate(
        term_cols: u16,
        term_rows: u16,
        video_w: u32,
        video_h: u32,
        scaling: ScalingMode,
        pixels_per_col: u32,
        pixels_per_row: u32,
    ) -> Self {
        if term_cols == 0 || term_rows == 0 || video_w == 0 || video_h == 0 {
            return Self {
                canvas_cols: term_cols,
                canvas_rows: term_rows,
                pixel_width: 1,
                pixel_height: 1,
                offset_cols: 0,
                offset_rows: 0,
                active_cols: 0,
                active_rows: 0,
            };
        }

        let total_pixel_w = term_cols as f64 * pixels_per_col as f64;
        let total_pixel_h = term_rows as f64 * pixels_per_row as f64;
        let video_ar = video_w as f64 / video_h as f64;
        // Typical terminal char aspect ratio: cell width / cell height ~= 0.5.
        // If pixels_per_col = 1 and pixels_per_row = 2, pixel aspect ratio is 1.0!
        let canvas_ar = total_pixel_w / total_pixel_h;

        let (target_pixel_w, target_pixel_h) = match scaling {
            ScalingMode::Stretch => (total_pixel_w, total_pixel_h),
            ScalingMode::Fit => {
                if canvas_ar > video_ar {
                    // Pillarbox: video is narrower than terminal
                    let h = total_pixel_h;
                    let w = h * video_ar;
                    (w, h)
                } else {
                    // Letterbox: video is wider than terminal
                    let w = total_pixel_w;
                    let h = w / video_ar;
                    (w, h)
                }
            }
            ScalingMode::Fill => {
                if canvas_ar > video_ar {
                    // Crop top/bottom
                    let w = total_pixel_w;
                    let h = w / video_ar;
                    (w, h)
                } else {
                    // Crop left/right
                    let h = total_pixel_h;
                    let w = h * video_ar;
                    (w, h)
                }
            }
        };

        let pixel_w = (target_pixel_w.round() as u32).max(pixels_per_col);
        let pixel_h = (target_pixel_h.round() as u32).max(pixels_per_row);

        // Snap pixel dimensions to cell boundaries
        let active_cols = ((pixel_w as f64 / pixels_per_col as f64).round() as u16).min(term_cols);
        let active_rows = ((pixel_h as f64 / pixels_per_row as f64).round() as u16).min(term_rows);

        let final_pixel_w = active_cols as u32 * pixels_per_col;
        let final_pixel_h = active_rows as u32 * pixels_per_row;

        let offset_cols = (term_cols.saturating_sub(active_cols)) / 2;
        let offset_rows = (term_rows.saturating_sub(active_rows)) / 2;

        Self {
            canvas_cols: term_cols,
            canvas_rows: term_rows,
            pixel_width: final_pixel_w,
            pixel_height: final_pixel_h,
            offset_cols,
            offset_rows,
            active_cols,
            active_rows,
        }
    }
}
