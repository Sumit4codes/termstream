use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use crossbeam_channel::{bounded, Receiver};
use crate::types::{MediaMetadata, VideoFrame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestPattern {
    Plasma,
    Matrix,
    Cube,
    Bars,
}

impl TestPattern {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "matrix" | "test:matrix" => Self::Matrix,
            "cube" | "test:cube" => Self::Cube,
            "bars" | "test:bars" => Self::Bars,
            _ => Self::Plasma,
        }
    }
}

pub struct ProceduralGenerator {
    stop_signal: Arc<AtomicBool>,
    pub frame_receiver: Receiver<VideoFrame>,
}

impl ProceduralGenerator {
    pub fn metadata(pattern: TestPattern) -> MediaMetadata {
        let title = match pattern {
            TestPattern::Plasma => "TermStream Demo: Psychedelic Plasma",
            TestPattern::Matrix => "TermStream Demo: Matrix Rain",
            TestPattern::Cube => "TermStream Demo: 3D Rotating Cube",
            TestPattern::Bars => "TermStream Demo: SMPTE Color Bars",
        };

        MediaMetadata {
            width: 320,
            height: 180,
            fps: 30.0,
            duration_secs: 120.0,
            has_audio: false,
            audio_sample_rate: 44100,
            audio_channels: 2,
            title: Some(title.to_string()),
        }
    }

    pub fn render_frame(
        pattern: TestPattern,
        target_w: u32,
        target_h: u32,
        t: f64,
        frame_buf: &mut [u8],
    ) {
        match pattern {
            TestPattern::Plasma => render_plasma(frame_buf, target_w, target_h, t),
            TestPattern::Matrix => render_matrix(frame_buf, target_w, target_h, t),
            TestPattern::Cube => render_cube(frame_buf, target_w, target_h, t),
            TestPattern::Bars => render_bars(frame_buf, target_w, target_h, t),
        }
    }

    pub fn start(
        pattern: TestPattern,
        target_w: u32,
        target_h: u32,
        fps: f64,
        seek_secs: f64,
    ) -> Self {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = bounded::<VideoFrame>(4);

        let stop_clone = Arc::clone(&stop_signal);
        let frame_duration = Duration::from_secs_f64(1.0 / fps);

        thread::Builder::new()
            .name("procedural-generator".into())
            .spawn(move || {
                let start_instant = Instant::now() - Duration::from_secs_f64(seek_secs);
                let mut frame_buf = vec![0u8; (target_w * target_h * 3) as usize];

                while !stop_clone.load(Ordering::Relaxed) {
                    let iter_start = Instant::now();
                    let t = start_instant.elapsed().as_secs_f64();

                    Self::render_frame(pattern, target_w, target_h, t, &mut frame_buf);

                    let frame = VideoFrame {
                        width: target_w,
                        height: target_h,
                        pts_secs: t,
                        data: frame_buf.clone(),
                    };

                    if sender.send(frame).is_err() {
                        break;
                    }

                    let elapsed = iter_start.elapsed();
                    if elapsed < frame_duration {
                        thread::sleep(frame_duration - elapsed);
                    }
                }
            })
            .expect("Failed to spawn procedural thread");

        Self {
            stop_signal,
            frame_receiver: receiver,
        }
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
    }
}

impl Drop for ProceduralGenerator {
    fn drop(&mut self) {
        self.stop();
    }
}

fn render_plasma(buf: &mut [u8], w: u32, h: u32, t: f64) {
    let fw = w as f64;
    let fh = h as f64;

    for y in 0..h {
        let fy = y as f64;
        let row_offset = (y * w * 3) as usize;

        for x in 0..w {
            let fx = x as f64;
            let v1 = (fx * 0.05 + t * 1.5).sin();
            let v2 = (fy * 0.06 - t * 1.2).sin();
            let cx = fx - fw * 0.5 + (t * 0.8).cos() * (fw * 0.3);
            let cy = fy - fh * 0.5 + (t * 0.9).sin() * (fh * 0.3);
            let dist = (cx * cx + cy * cy).sqrt() * 0.05;
            let v3 = (dist - t * 2.0).sin();
            let v = (v1 + v2 + v3) / 3.0; // [-1.0, 1.0]

            let r = (((v * std::f64::consts::PI).sin() * 0.5 + 0.5) * 255.0) as u8;
            let g = ((((v + 0.33) * std::f64::consts::PI).sin() * 0.5 + 0.5) * 255.0) as u8;
            let b = ((((v + 0.66) * std::f64::consts::PI).sin() * 0.5 + 0.5) * 255.0) as u8;

            let idx = row_offset + (x * 3) as usize;
            buf[idx] = r;
            buf[idx + 1] = g;
            buf[idx + 2] = b;
        }
    }
}

fn render_matrix(buf: &mut [u8], w: u32, h: u32, t: f64) {
    // Fill background with decaying dark green
    for byte in buf.iter_mut() {
        *byte = (*byte as f32 * 0.88) as u8;
    }

    // Moving drops per column
    let num_columns = (w / 4).max(1);
    for col in 0..num_columns {
        let speed = 15.0 + ((col * 17) % 25) as f64;
        let y_pos = ((t * speed + (col as f64 * 31.0)) % (h as f64 * 1.5)) as i32;

        for tail in 0..12 {
            let y = y_pos - tail;
            if y >= 0 && y < h as i32 {
                let px = col * 4;
                if px < w {
                    let idx = ((y as u32 * w + px) * 3) as usize;
                    if tail == 0 {
                        // Bright white/cyan head
                        buf[idx] = 220;
                        buf[idx + 1] = 255;
                        buf[idx + 2] = 220;
                    } else {
                        // Green tail
                        let brightness = (255 - tail * 20).max(40) as u8;
                        buf[idx] = 10;
                        buf[idx + 1] = brightness;
                        buf[idx + 2] = 30;
                    }
                }
            }
        }
    }
}

fn render_cube(buf: &mut [u8], w: u32, h: u32, t: f64) {
    buf.fill(15); // Dark gray background

    // 3D Cube vertices
    let vertices: [[f64; 3]; 8] = [
        [-1.0, -1.0, -1.0],
        [ 1.0, -1.0, -1.0],
        [ 1.0,  1.0, -1.0],
        [-1.0,  1.0, -1.0],
        [-1.0, -1.0,  1.0],
        [ 1.0, -1.0,  1.0],
        [ 1.0,  1.0,  1.0],
        [-1.0,  1.0,  1.0],
    ];

    let edges = [
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];

    let rot_x = t * 1.2;
    let rot_y = t * 1.7;
    let rot_z = t * 0.8;

    let (sx, cx) = (rot_x.sin(), rot_x.cos());
    let (sy, cy) = (rot_y.sin(), rot_y.cos());
    let (sz, cz) = (rot_z.sin(), rot_z.cos());

    let scale = (h as f64 * 0.38).min(w as f64 * 0.38);
    let center_x = w as f64 * 0.5;
    let center_y = h as f64 * 0.5;

    let mut projected: [[f64; 2]; 8] = [[0.0, 0.0]; 8];

    for (i, v) in vertices.iter().enumerate() {
        // Rotate Y
        let x1 = v[0] * cy + v[2] * sy;
        let y1 = v[1];
        let z1 = -v[0] * sy + v[2] * cy;

        // Rotate X
        let x2 = x1;
        let y2 = y1 * cx - z1 * sx;
        let z2 = y1 * sx + z1 * cx;

        // Rotate Z
        let x3 = x2 * cz - y2 * sz;
        let y3 = x2 * sz + y2 * cz;
        let z3 = z2 + 3.5; // camera distance

        let fov = 2.5;
        let px = center_x + (x3 * fov / z3) * scale;
        let py = center_y + (y3 * fov / z3) * scale;
        projected[i] = [px, py];
    }

    // Draw edges with rainbow colors
    for (i, &(p1, p2)) in edges.iter().enumerate() {
        let pt1 = projected[p1];
        let pt2 = projected[p2];

        let hue = ((i as f64 / 12.0) + t * 0.2) % 1.0;
        let (r, g, b) = hsv_to_rgb(hue, 0.9, 1.0);

        draw_line(buf, w, h, pt1[0] as i32, pt1[1] as i32, pt2[0] as i32, pt2[1] as i32, r, g, b);
    }
}

fn draw_line(buf: &mut [u8], w: u32, h: u32, mut x0: i32, mut y0: i32, x1: i32, y1: i32, r: u8, g: u8, b: u8) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 >= 0 && x0 < w as i32 && y0 >= 0 && y0 < h as i32 {
            let idx = ((y0 as u32 * w + x0 as u32) * 3) as usize;
            if idx + 2 < buf.len() {
                buf[idx] = r;
                buf[idx + 1] = g;
                buf[idx + 2] = b;
            }
        }

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }
}

fn render_bars(buf: &mut [u8], w: u32, h: u32, t: f64) {
    let colors = [
        (192, 192, 192), // Gray
        (192, 192, 0),   // Yellow
        (0, 192, 192),   // Cyan
        (0, 192, 0),     // Green
        (192, 0, 192),   // Magenta
        (192, 0, 0),     // Red
        (0, 0, 192),     // Blue
    ];

    let bar_w = (w as usize) / colors.len();
    let top_h = (h as usize * 3) / 4;

    for y in 0..top_h {
        let row_offset = y * (w as usize) * 3;
        for (i, &(r, g, b)) in colors.iter().enumerate() {
            let x_start = i * bar_w;
            let x_end = if i == colors.len() - 1 { w as usize } else { (i + 1) * bar_w };

            for x in x_start..x_end {
                let idx = row_offset + x * 3;
                buf[idx] = r;
                buf[idx + 1] = g;
                buf[idx + 2] = b;
            }
        }
    }

    // Moving sync pip at bottom
    let bot_start = top_h;
    let pip_pos = ((t * 0.8) % 1.0 * w as f64) as usize;

    for y in bot_start..(h as usize) {
        let row_offset = y * (w as usize) * 3;
        for x in 0..(w as usize) {
            let idx = row_offset + x * 3;
            if (x as i32 - pip_pos as i32).abs() < 5 {
                buf[idx] = 255;
                buf[idx + 1] = 255;
                buf[idx + 2] = 255;
            } else {
                buf[idx] = 30;
                buf[idx + 1] = 30;
                buf[idx + 2] = 30;
            }
        }
    }
}

fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f64;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}
