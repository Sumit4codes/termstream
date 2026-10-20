use std::fmt::Write;
use crate::render::{ColorLut, RenderGeometry};
use crate::types::VideoFrame;

const B64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const CHUNK_RAW_SIZE: usize = 3072; // 3072 raw bytes -> exactly 4096 base64 chars (Kitty max chunk size)

/// Fast Base64 chunk encoder writing directly into String buffer
#[inline(always)]
fn encode_b64_chunk(raw: &[u8], out: &mut String) {
    let mut i = 0;
    let len = raw.len();
    while i + 3 <= len {
        let b0 = raw[i];
        let b1 = raw[i + 1];
        let b2 = raw[i + 2];
        i += 3;

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);
        out.push(B64_CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push(B64_CHARS[(n & 0x3F) as usize] as char);
    }

    if i + 1 == len {
        let b0 = raw[i];
        let n = (b0 as u32) << 16;
        out.push(B64_CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push('=');
        out.push('=');
    } else if i + 2 == len {
        let b0 = raw[i];
        let b1 = raw[i + 1];
        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8);
        out.push(B64_CHARS[((n >> 18) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((n >> 12) & 0x3F) as usize] as char);
        out.push(B64_CHARS[((n >> 6) & 0x3F) as usize] as char);
        out.push('=');
    }
}

/// Render video frame using the Kitty Graphics Protocol (direct 24-bit RGB TrueColor)
pub fn render_kitty(
    frame: &VideoFrame,
    geom: &RenderGeometry,
    lut: &ColorLut,
    buffer: &mut String,
) {
    let w = frame.width;
    let h = frame.height;
    if w == 0 || h == 0 || frame.data.is_empty() {
        return;
    }

    let act_cols = geom.active_cols.max(1);
    let act_rows = geom.active_rows.max(1);
    let row = geom.offset_rows + 1;
    let col = geom.offset_cols + 1;

    // Delete previous frame placement with ID 1 to avoid unbounded placement accumulation
    buffer.push_str("\x1b_Ga=d,d=i,i=1\x1b\\");

    // Move cursor to top-left of active video region
    let _ = write!(buffer, "\x1b[{};{}H", row, col);

    // Apply color LUT if active
    let transformed: Vec<u8>;
    let pixel_bytes: &[u8] = if lut.is_identity() {
        &frame.data
    } else {
        let pixel_count = (w * h) as usize;
        let mut buf = vec![0u8; pixel_count * 3];
        for i in 0..pixel_count {
            let src_idx = i * 3;
            if src_idx + 2 < frame.data.len() {
                let r = frame.data[src_idx];
                let g = frame.data[src_idx + 1];
                let b = frame.data[src_idx + 2];
                let (nr, ng, nb) = lut.transform_rgb(r, g, b);
                buf[src_idx] = nr;
                buf[src_idx + 1] = ng;
                buf[src_idx + 2] = nb;
            }
        }
        transformed = buf;
        &transformed
    };

    let total_bytes = pixel_bytes.len();
    let num_chunks = (total_bytes + CHUNK_RAW_SIZE - 1) / CHUNK_RAW_SIZE;

    if num_chunks <= 1 {
        // Fits entirely in a single chunk
        let _ = write!(
            buffer,
            "\x1b_Ga=T,f=24,s={},v={},c={},r={},z=-1,q=2,i=1,m=0;",
            w, h, act_cols, act_rows
        );
        encode_b64_chunk(pixel_bytes, buffer);
        buffer.push_str("\x1b\\");
    } else {
        // Multi-chunk transmission (<= 4096 base64 chars per escape sequence)
        for chunk_idx in 0..num_chunks {
            let start = chunk_idx * CHUNK_RAW_SIZE;
            let end = (start + CHUNK_RAW_SIZE).min(total_bytes);
            let chunk_data = &pixel_bytes[start..end];
            let is_last = chunk_idx + 1 == num_chunks;

            if chunk_idx == 0 {
                let _ = write!(
                    buffer,
                    "\x1b_Ga=T,f=24,s={},v={},c={},r={},z=-1,q=2,i=1,m=1;",
                    w, h, act_cols, act_rows
                );
            } else {
                let _ = write!(buffer, "\x1b_Gm={};", if is_last { 0 } else { 1 });
            }

            encode_b64_chunk(chunk_data, buffer);
            buffer.push_str("\x1b\\");
        }
    }
}
