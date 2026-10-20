use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use anyhow::{Context, Result};
use crossbeam_channel::{bounded, Receiver, Sender};
use rodio::{OutputStream, OutputStreamHandle, Sink, Source};

pub struct PcmSource {
    receiver: Receiver<i16>,
    current_sample: i16,
    sample_rate: u32,
    channels: u16,
}

impl PcmSource {
    pub fn new(receiver: Receiver<i16>, sample_rate: u32, channels: u16) -> Self {
        Self {
            receiver,
            current_sample: 0,
            sample_rate,
            channels,
        }
    }
}

impl Iterator for PcmSource {
    type Item = i16;

    fn next(&mut self) -> Option<Self::Item> {
        match self.receiver.recv_timeout(Duration::from_millis(50)) {
            Ok(s) => {
                self.current_sample = s;
                Some(s)
            }
            Err(_) => {
                // If stream is empty or stalled, output silence instead of killing the sink
                Some(0)
            }
        }
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
    _handle: Option<OutputStreamHandle>,
    sink: Option<Arc<Sink>>,
    child: Option<Child>,
    stop_signal: Arc<AtomicBool>,
    sender: Option<Sender<i16>>,
    path: String,
    volume: f32,
    is_muted: bool,
    enabled: bool,
}

impl AudioPlayer {
    pub fn new(path: String, enabled: bool) -> Self {
        if !enabled {
            return Self {
                _stream: None,
                _handle: None,
                sink: None,
                child: None,
                stop_signal: Arc::new(AtomicBool::new(false)),
                sender: None,
                path,
                volume: 1.0,
                is_muted: false,
                enabled: false,
            };
        }

        let (stream, handle, sink) = match OutputStream::try_default() {
            Ok((stream, handle)) => match Sink::try_new(&handle) {
                Ok(sink) => (Some(stream), Some(handle), Some(Arc::new(sink))),
                Err(_) => (None, None, None),
            },
            Err(_) => (None, None, None),
        };

        Self {
            _stream: stream,
            _handle: handle,
            sink,
            child: None,
            stop_signal: Arc::new(AtomicBool::new(false)),
            sender: None,
            path,
            volume: 1.0,
            is_muted: false,
            enabled: true,
        }
    }

    pub fn start(&mut self, seek_secs: f64) -> Result<()> {
        if !self.enabled || self.sink.is_none() {
            return Ok(());
        }

        self.stop();

        let stop_signal = Arc::new(AtomicBool::new(false));
        self.stop_signal = Arc::clone(&stop_signal);

        let (tx, rx) = bounded::<i16>(44100 * 4); // ~2 seconds of buffer
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

        let source = PcmSource::new(rx, 44100, 2);
        if let Some(sink) = &self.sink {
            sink.append(source);
            sink.set_volume(if self.is_muted { 0.0 } else { self.volume });
            sink.play();
        }

        thread::Builder::new()
            .name("audio-decoder".into())
            .spawn(move || {
                let mut chunk = [0u8; 4096];
                while !stop_signal.load(Ordering::Relaxed) {
                    match stdout.read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => {
                            let samples = n / 2;
                            for i in 0..samples {
                                let sample = i16::from_le_bytes([chunk[i * 2], chunk[i * 2 + 1]]);
                                if tx.send(sample).is_err() {
                                    return;
                                }
                            }
                        }
                        Err(_) => break,
                    }
                }
            })?;

        self.child = Some(child);
        Ok(())
    }

    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.5);
        if let Some(sink) = &self.sink {
            if !self.is_muted {
                sink.set_volume(self.volume);
            }
        }
    }

    pub fn get_volume(&self) -> f32 {
        self.volume
    }

    pub fn toggle_mute(&mut self) -> bool {
        self.is_muted = !self.is_muted;
        if let Some(sink) = &self.sink {
            sink.set_volume(if self.is_muted { 0.0 } else { self.volume });
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
        if let Some(sink) = &self.sink {
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
