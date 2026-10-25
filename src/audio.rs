use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

pub struct PcmSource {
    receiver: Receiver<Vec<i16>>,
    current_chunk: Vec<i16>,
    chunk_index: usize,
    sample_rate: u32,
    channels: u16,
    samples_played: Arc<AtomicU64>,
}

impl PcmSource {
    pub fn new(
        receiver: Receiver<Vec<i16>>,
        sample_rate: u32,
        channels: u16,
        samples_played: Arc<AtomicU64>,
    ) -> Self {
        Self {
            receiver,
            current_chunk: Vec::new(),
            chunk_index: 0,
            sample_rate,
            channels,
            samples_played,
        }
    }
}

impl Iterator for PcmSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        while self.chunk_index >= self.current_chunk.len() {
            match self.receiver.try_recv() {
                Ok(chunk) => {
                    self.current_chunk = chunk;
                    self.chunk_index = 0;
                }
                Err(TryRecvError::Empty) => {
                    // Small underrun: output silence without hanging audio callback.
                    // Do NOT increment samples_played so PTS stays locked to real stream samples.
                    return Some(0);
                }
                Err(TryRecvError::Disconnected) => {
                    return None;
                }
            }
        }

        let sample = self.current_chunk[self.chunk_index];
        self.chunk_index += 1;
        self.samples_played.fetch_add(1, Ordering::Relaxed);
        Some(sample)
    }
}

impl Source for PcmSource {
    fn current_frame_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

pub struct AudioPlayer {
    _stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    child: Option<Child>,
    stop_signal: Arc<AtomicBool>,
    sender: Option<Sender<Vec<i16>>>,
    samples_played: Arc<AtomicU64>,
    base_seek_ms: Arc<AtomicU64>,
    path: String,
    volume: f32,
    is_muted: bool,
    enabled: bool,
}

impl AudioPlayer {
    /// Preamp boost factor applied to decoded PCM audio (+2.6 dB)
    pub const PREAMP_GAIN: f32 = 1.35;

    pub fn new(path: String, enabled: bool) -> Self {
        let samples_played = Arc::new(AtomicU64::new(0));
        let base_seek_ms = Arc::new(AtomicU64::new(0));

        if !enabled {
            return Self {
                _stream: None,
                handle: None,
                sink: None,
                child: None,
                stop_signal: Arc::new(AtomicBool::new(false)),
                sender: None,
                samples_played,
                base_seek_ms,
                path,
                volume: 1.2,
                is_muted: false,
                enabled: false,
            };
        }

        let (stream, handle, sink) = match OutputStream::try_default() {
            Ok((stream, handle)) => {
                let sink = Sink::try_new(&handle).ok();
                (Some(stream), Some(handle), sink)
            }
            Err(_) => (None, None, None),
        };

        Self {
            _stream: stream,
            handle,
            sink,
            child: None,
            stop_signal: Arc::new(AtomicBool::new(false)),
            sender: None,
            samples_played,
            base_seek_ms,
            path,
            volume: 1.2,
            is_muted: false,
            enabled: true,
        }
    }

    #[inline]
    pub fn effective_volume(&self) -> f32 {
        self.volume * Self::PREAMP_GAIN
    }

    pub fn start(&mut self, seek_secs: f64) -> Result<()> {
        if !self.enabled || self.handle.is_none() {
            return Ok(());
        }

        self.stop();

        let stop_signal = Arc::new(AtomicBool::new(false));
        self.stop_signal = Arc::clone(&stop_signal);

        self.base_seek_ms
            .store((seek_secs * 1000.0) as u64, Ordering::SeqCst);
        self.samples_played.store(0, Ordering::SeqCst);

        // Bounded queue of 64 chunks (approx 3 seconds of audio buffer)
        let (tx, rx) = bounded::<Vec<i16>>(64);
        self.sender = Some(tx.clone());

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-v", "error"]);
        if seek_secs > 0.0 {
            cmd.args(["-ss", &format!("{:.3}", seek_secs)]);
        }
        cmd.args([
            "-i", &self.path,
            "-vn",
            "-sn",
            "-f", "s16le",
            "-ac", "2",
            "-ar", "44100",
            "pipe:1",
        ]);
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::null());
        cmd.stdin(Stdio::null());

        let mut child = cmd.spawn().context("Failed to spawn ffmpeg audio decoder")?;
        let mut stdout = child.stdout.take().context("Failed to take audio stdout")?;

        thread::Builder::new()
            .name("audio-decoder".into())
            .spawn(move || {
                let mut chunk_bytes = [0u8; 8192];
                while !stop_signal.load(Ordering::Relaxed) {
                    let mut read_bytes = 0;
                    while read_bytes < chunk_bytes.len() {
                        match stdout.read(&mut chunk_bytes[read_bytes..]) {
                            Ok(0) => break,
                            Ok(n) => read_bytes += n,
                            Err(_) => break,
                        }
                    }

                    if read_bytes == 0 {
                        break;
                    }

                    let sample_count = read_bytes / 2;
                    let mut samples = Vec::with_capacity(sample_count);
                    for i in 0..sample_count {
                        samples.push(i16::from_le_bytes([
                            chunk_bytes[i * 2],
                            chunk_bytes[i * 2 + 1],
                        ]));
                    }

                    if tx.send(samples).is_err() {
                        return;
                    }
                }
            })?;

        // Pre-buffer initial audio so playback begins immediately without underrun
        let wait_start = std::time::Instant::now();
        while rx.is_empty() && wait_start.elapsed() < Duration::from_millis(1500) {
            thread::sleep(Duration::from_millis(5));
        }

        if let Some(handle) = &self.handle {
            if let Ok(sink) = Sink::try_new(handle) {
                let source = PcmSource::new(rx, 44100, 2, Arc::clone(&self.samples_played));
                sink.append(source);
                sink.set_volume(if self.is_muted { 0.0 } else { self.effective_volume() });
                sink.play();
                self.sink = Some(sink);
            }
        }

        self.child = Some(child);
        Ok(())
    }

    pub fn current_pts(&self) -> Option<f64> {
        if !self.enabled || self.sink.is_none() {
            return None;
        }
        let samples = self.samples_played.load(Ordering::Relaxed);
        let base_ms = self.base_seek_ms.load(Ordering::Relaxed);
        // Stereo: 2 samples per audio frame at 44100 Hz
        let audio_secs = samples as f64 / (44100.0 * 2.0);
        // Compensate for soundcard output buffer latency (~20ms in CPAL ring buffer)
        let latency_compensation = 0.020;
        let adj_audio_secs = (audio_secs - latency_compensation).max(0.0);
        Some((base_ms as f64 / 1000.0) + adj_audio_secs)
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 3.0);
        if let Some(sink) = &self.sink {
            if !self.is_muted {
                sink.set_volume(self.effective_volume());
            }
        }
    }

    pub fn get_volume(&self) -> f32 {
        self.volume
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.is_muted = !self.is_muted;
        if let Some(sink) = &self.sink {
            sink.set_volume(if self.is_muted { 0.0 } else { self.effective_volume() });
        }
        self.is_muted
    }

    #[allow(dead_code)]
    pub fn is_muted(&self) -> bool {
        self.is_muted
    }

    pub fn pause(&self) {
        if let Some(sink) = &self.sink {
            sink.pause();
        }
    }

    pub fn resume(&self) {
        if let Some(sink) = &self.sink {
            sink.play();
        }
    }

    pub fn stop(&mut self) {
        self.stop_signal.store(true, Ordering::Relaxed);
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.sender = None;
    }
}

impl Drop for AudioPlayer {
    fn drop(&mut self) {
        self.stop();
    }
}
