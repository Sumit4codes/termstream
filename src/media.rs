use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver};
use serde_json::Value;

use crate::types::{MediaMetadata, VideoFrame};

pub struct MediaProbe;

impl MediaProbe {
    pub fn probe(path: &str) -> Result<MediaMetadata> {
        let output = Command::new("ffprobe")
            .args([
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
                path,
            ])
            .output()
            .context("Failed to run ffprobe. Ensure ffmpeg and ffprobe are installed.")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("ffprobe failed: {}", stderr);
        }

        let json: Value = serde_json::from_slice(&output.stdout)
            .context("Failed to parse ffprobe JSON output")?;

        let mut width = 640;
        let mut height = 360;
        let mut fps = 30.0;
        let mut has_audio = false;
        let mut audio_sample_rate = 44100;
        let mut audio_channels = 2;
        let mut duration_secs = 0.0;

        if let Some(format) = json.get("format") {
            if let Some(dur_str) = format.get("duration").and_then(|v| v.as_str()) {
                duration_secs = dur_str.parse::<f64>().unwrap_or(0.0);
            }
        }

        if let Some(streams) = json.get("streams").and_then(|v| v.as_array()) {
            for stream in streams {
                let codec_type = stream.get("codec_type").and_then(|v| v.as_str()).unwrap_or("");
                if codec_type == "video" && width == 640 && height == 360 {
                    if let Some(w) = stream.get("width").and_then(|v| v.as_u64()) {
                        width = w as u32;
                    }
                    if let Some(h) = stream.get("height").and_then(|v| v.as_u64()) {
                        height = h as u32;
                    }
                    if let Some(r_fps) = stream.get("r_frame_rate").and_then(|v| v.as_str()) {
                        if let Some((num, den)) = r_fps.split_once('/') {
                            let n: f64 = num.parse().unwrap_or(30.0);
                            let d: f64 = den.parse().unwrap_or(1.0);
                            if d > 0.0 {
                                fps = n / d;
                            }
                        }
                    }
                    if duration_secs == 0.0 {
                        if let Some(dur_str) = stream.get("duration").and_then(|v| v.as_str()) {
                            duration_secs = dur_str.parse::<f64>().unwrap_or(0.0);
                        }
                    }
                } else if codec_type == "audio" {
                    has_audio = true;
                    if let Some(sr) = stream.get("sample_rate").and_then(|v| v.as_str()) {
                        audio_sample_rate = sr.parse().unwrap_or(44100);
                    }
                    if let Some(ch) = stream.get("channels").and_then(|v| v.as_u64()) {
                        audio_channels = ch as u16;
                    }
                }
            }
        }

        let title = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.to_string());

        Ok(MediaMetadata {
            width,
            height,
            fps: if fps <= 0.0 || fps > 120.0 { 30.0 } else { fps },
            duration_secs,
            has_audio,
            audio_sample_rate,
            audio_channels,
            title,
        })
    }
}

pub struct VideoStreamDecoder {
    child: Option<Child>,
    stop_signal: Arc<AtomicBool>,
    pub frame_receiver: Receiver<VideoFrame>,
}

impl VideoStreamDecoder {
    pub fn start(
        path: String,
        target_width: u32,
        target_height: u32,
        fps: f64,
        seek_secs: f64,
    ) -> Result<Self> {
        let stop_signal = Arc::new(AtomicBool::new(false));
        let (sender, receiver) = bounded::<VideoFrame>(4);

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-v", "error"]);

        if seek_secs > 0.0 {
            cmd.args(["-ss", &format!("{:.3}", seek_secs)]);
        }

        cmd.args([
            "-i", &path,
            "-an",
            "-sn",
            "-f", "rawvideo",
            "-pix_fmt", "rgb24",
            "-s", &format!("{}x{}", target_width, target_height),
            "-r", &format!("{:.3}", fps),
            "pipe:1",
        ]);

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        cmd.stdin(Stdio::null());

        let mut child = cmd.spawn().context("Failed to spawn ffmpeg video decoder")?;
        let mut stdout = child.stdout.take().context("Failed to open ffmpeg stdout")?;

        let stop_clone = Arc::clone(&stop_signal);
        let frame_size = (target_width * target_height * 3) as usize;

        thread::Builder::new()
            .name("video-decoder".into())
            .spawn(move || {
                let mut buffer = vec![0u8; frame_size];
                let mut frame_idx: u64 = 0;
                let frame_duration = 1.0 / fps;

                while !stop_clone.load(Ordering::Relaxed) {
                    let mut read_bytes = 0;
                    let mut eof = false;

                    while read_bytes < frame_size {
                        match stdout.read(&mut buffer[read_bytes..]) {
                            Ok(0) => {
                                eof = true;
                                break;
                            }
                            Ok(n) => {
                                read_bytes += n;
                            }
                            Err(_) => {
                                eof = true;
                                break;
                            }
                        }
                    }

                    if eof || read_bytes < frame_size {
                        break;
                    }

                    let frame = VideoFrame {
                        width: target_width,
                        height: target_height,
                        pts_secs: seek_secs + (frame_idx as f64 * frame_duration),
                        data: buffer.clone(),
                    };

                    frame_idx += 1;

                    if sender.send(frame).is_err() {
                        // Consumer dropped
                        break;
                    }
                }
            })?;

        Ok(Self {
            child: Some(child),
            stop_signal,
            frame_receiver: receiver,
        })
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Drop for VideoStreamDecoder {
    fn drop(&mut self) {
        self.stop();
    }
}
