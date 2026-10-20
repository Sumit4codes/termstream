use std::io::{stdout, Write};
use std::time::{Duration, Instant};
use anyhow::Result;
use clap::Parser;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, size as terminal_size,
        EnterAlternateScreen, LeaveAlternateScreen,
    },
};

use termstream::audio::AudioPlayer;
use termstream::clock::Clock;
use termstream::generator::{ProceduralGenerator, TestPattern};
use termstream::media::{MediaProbe, VideoStreamDecoder};
use termstream::render::{
    ascii::render_ascii, braille::render_braille, half_block::render_half_block,
    quadrant::render_quadrant, ColorLut, RenderGeometry,
};
use termstream::types::{ColorFilter, MediaMetadata, OsdMode, RenderMode, ScalingMode, VideoFrame};
use termstream::ui::{self, render_ui, UiState};

#[derive(Parser, Debug)]
#[command(
    name = "termstream",
    version = "0.1.0",
    about = "High-performance Terminal Video Player from scratch in Rust with A/V sync and TrueColor rendering"
)]
struct Args {
    /// Video file path, URL, or demo stream ("test:plasma", "test:matrix", "test:cube", "test:bars")
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    /// Render mode
    #[arg(short, long, value_enum, default_value = "half-block")]
    mode: RenderMode,

    /// Scaling mode
    #[arg(short, long, value_enum, default_value = "fit")]
    aspect: ScalingMode,

    /// Color filter
    #[arg(short, long, value_enum, default_value = "none")]
    filter: ColorFilter,

    /// Disable audio playback
    #[arg(long)]
    no_audio: bool,

    /// Loop video continuously
    #[arg(short, long)]
    r#loop: bool,

    /// Initial volume percentage (0 - 300)
    #[arg(long, default_value = "120")]
    volume: u32,

    /// Target frame rate override
    #[arg(long)]
    fps: Option<f64>,

    /// Stream classic "Bad Apple!!" YouTube benchmark
    #[arg(long)]
    bad_apple: bool,

    /// Stream Rick Astley's "Never Gonna Give You Up"
    #[arg(long)]
    rickroll: bool,

    /// Run built-in procedural demo pattern
    #[arg(long)]
    test: bool,

    /// Print video metadata and exit
    #[arg(long)]
    info: bool,
}

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let mut out = stdout();
        let _ = execute!(out, Show, LeaveAlternateScreen);
        let _ = disable_raw_mode();
        let _ = out.flush();
    }
}

enum VideoSource {
    Ffmpeg {
        video_path: String,
        audio_path: String,
    },
    Procedural(TestPattern),
}

fn resolve_input_path(raw: &str) -> String {
    let trimmed = raw.trim();
    if std::path::Path::new(trimmed).exists() {
        return trimmed.to_string();
    }
    // Handle paths that got line-wrapped during terminal copy-paste
    let unwrapped = raw
        .lines()
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join("");
    if std::path::Path::new(&unwrapped).exists() {
        return unwrapped;
    }
    let unquoted = trimmed.trim_matches('\'').trim_matches('"');
    if std::path::Path::new(unquoted).exists() {
        return unquoted.to_string();
    }
    trimmed.to_string()
}

fn main() -> Result<()> {
    let args = Args::parse();

    let (source, metadata) = if args.bad_apple {
        println!("Fetching Bad Apple!! stream via yt-dlp...");
        let stream = termstream::ytdl::YtDlp::resolve(termstream::ytdl::BAD_APPLE_URL)?;
        println!("Streaming: {}", stream.title);
        let mut meta = MediaProbe::probe(&stream.video_url)?;
        meta.title = Some(stream.title);
        meta.has_audio = true;
        (VideoSource::Ffmpeg { video_path: stream.video_url, audio_path: stream.audio_url }, meta)
    } else if args.rickroll {
        println!("Fetching Rick Astley stream via yt-dlp...");
        let stream = termstream::ytdl::YtDlp::resolve(termstream::ytdl::RICKROLL_URL)?;
        println!("Streaming: {}", stream.title);
        let mut meta = MediaProbe::probe(&stream.video_url)?;
        meta.title = Some(stream.title);
        meta.has_audio = true;
        (VideoSource::Ffmpeg { video_path: stream.video_url, audio_path: stream.audio_url }, meta)
    } else {
        match &args.input {
            Some(s) if s.starts_with("test:") || s == "matrix" || s == "cube" || s == "bars" || s == "plasma" => {
                let pattern = TestPattern::parse(s);
                (VideoSource::Procedural(pattern), ProceduralGenerator::metadata(pattern))
            }
            Some(raw) if termstream::ytdl::YtDlp::is_web_url(raw) => {
                println!("Fetching web video stream via yt-dlp: {}...", raw);
                let stream = termstream::ytdl::YtDlp::resolve(raw)?;
                println!("Streaming: {}", stream.title);
                let mut meta = MediaProbe::probe(&stream.video_url)?;
                meta.title = Some(stream.title);
                meta.has_audio = true;
                if args.info {
                    println!("TermStream Media Info:");
                    println!("  Title:       {}", meta.title.as_deref().unwrap_or("Unknown"));
                    println!("  Resolution:  {}x{}", meta.width, meta.height);
                    println!("  FPS:         {:.2}", meta.fps);
                    println!("  Duration:    {:.2}s ({})", meta.duration_secs, ui::format_time(meta.duration_secs));
                    println!("  Audio:       {}", if meta.has_audio { format!("{} Hz, {} channels", meta.audio_sample_rate, meta.audio_channels) } else { "No audio".to_string() });
                    return Ok(());
                }
                (VideoSource::Ffmpeg { video_path: stream.video_url, audio_path: stream.audio_url }, meta)
            }
            Some(raw_path) => {
                let path = resolve_input_path(raw_path);
                let meta = MediaProbe::probe(&path)?;
                if args.info {
                    println!("TermStream Media Info:");
                    println!("  Title:       {}", meta.title.as_deref().unwrap_or("Unknown"));
                    println!("  Resolution:  {}x{}", meta.width, meta.height);
                    println!("  FPS:         {:.2}", meta.fps);
                    println!("  Duration:    {:.2}s ({})", meta.duration_secs, ui::format_time(meta.duration_secs));
                    println!("  Audio:       {}", if meta.has_audio { format!("{} Hz, {} channels", meta.audio_sample_rate, meta.audio_channels) } else { "No audio".to_string() });
                    return Ok(());
                }
                (VideoSource::Ffmpeg { video_path: path.clone(), audio_path: path }, meta)
            }
            None => {
                if args.test || args.input.is_none() {
                    let pattern = TestPattern::Plasma;
                    (VideoSource::Procedural(pattern), ProceduralGenerator::metadata(pattern))
                } else {
                    anyhow::bail!("No input provided. Usage: termstream <FILE_OR_URL> or termstream --test");
                }
            }
        }
    };

    run_player(args, source, metadata)
}

fn run_player(args: Args, source: VideoSource, meta: MediaMetadata) -> Result<()> {
    enable_raw_mode()?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen, Hide)?;
    let _cleanup = TerminalCleanup;

    let (mut cols, mut rows) = terminal_size().unwrap_or((80, 24));

    let mut current_mode = args.mode;
    let mut current_scaling = args.aspect;
    let mut current_filter = args.filter;
    let mut brightness: i32 = 0;
    let mut contrast: f32 = 1.0;
    let mut invert = false;
    let mut is_looping = args.r#loop;
    let mut osd_mode = OsdMode::AutoHide;
    let mut last_interaction = Instant::now();

    let fps = args.fps.unwrap_or(meta.fps).clamp(1.0, 60.0);
    let mut clock = Clock::new(0.0);
    let mut lut = ColorLut::new(brightness, contrast, invert, current_filter);

    let initial_vol = (args.volume as f32 / 100.0).clamp(0.0, 3.0);
    let mut audio = match &source {
        VideoSource::Ffmpeg { audio_path, .. } if meta.has_audio && !args.no_audio => {
            let mut a = AudioPlayer::new(audio_path.clone(), true);
            a.set_volume(initial_vol);
            a
        }
        _ => AudioPlayer::new(String::new(), false),
    };

    let mut ui_state = UiState {
        current_pts: 0.0,
        duration: meta.duration_secs,
        is_paused: false,
        volume: initial_vol,
        is_muted: false,
        is_looping,
        render_mode: current_mode,
        scaling_mode: current_scaling,
        color_filter: current_filter,
        brightness,
        contrast,
        invert,
        fps,
        show_help: false,
        osd_mode,
        message: None,
    };

    // Calculate pixel dimensions needed for current mode
    let (px_per_col, px_per_row) = current_mode.subpixels();

    let mut geom = RenderGeometry::calculate(
        cols,
        rows.saturating_sub(1), // Leave 1 line for status bar
        meta.width,
        meta.height,
        current_scaling,
        px_per_col,
        px_per_row,
    );

    let start_decoders = |source: &VideoSource, geom: &RenderGeometry, fps: f64, seek: f64| {
        match source {
            VideoSource::Ffmpeg { video_path, .. } => {
                let dec = VideoStreamDecoder::start(
                    video_path.clone(),
                    geom.pixel_width,
                    geom.pixel_height,
                    fps,
                    seek,
                );
                (Some(dec.expect("Failed to start video decoder")), None)
            }
            VideoSource::Procedural(pattern) => {
                let gen = ProceduralGenerator::start(
                    *pattern,
                    geom.pixel_width,
                    geom.pixel_height,
                    fps,
                    seek,
                );
                (None, Some(gen))
            }
        }
    };

    let (dec, gen) = start_decoders(&source, &geom, fps, 0.0);
    let mut ffmpeg_decoder = dec;
    let mut proc_generator = gen;
    let _ = audio.start(0.0);
    clock.start();

    let mut current_frame: Option<VideoFrame> = None;
    let mut queued_frame: Option<VideoFrame> = None;
    let mut buffer = String::with_capacity((cols as usize * rows as usize) * 35);
    let mut frame_count: u64 = 0;
    let mut fps_timer = Instant::now();
    let frame_interval = 1.0 / fps;

    loop {
        // Compute Current Master Clock PTS
        let clock_pts = clock.get_pts();
        let current_pts = if !clock.is_paused() {
            if let Some(apts) = audio.current_pts() {
                if (apts - clock_pts).abs() < 1.0 {
                    apts
                } else {
                    clock_pts
                }
            } else {
                clock_pts
            }
        } else {
            clock_pts
        };
        ui_state.current_pts = current_pts;
        let mut needs_render = false;

        // Periodically verify terminal size in case resize event was missed by terminal (e.g. font zoom in/out)
        if let Ok((curr_cols, curr_rows)) = terminal_size() {
            if curr_cols > 0 && curr_rows > 0 && (curr_cols != cols || curr_rows != rows) {
                cols = curr_cols;
                rows = curr_rows;
                let (px_c, px_r) = current_mode.subpixels();
                geom = RenderGeometry::calculate(
                    cols,
                    rows.saturating_sub(1),
                    meta.width,
                    meta.height,
                    current_scaling,
                    px_c,
                    px_r,
                );
                let pts = clock.get_pts();
                let (dec, gen) = start_decoders(&source, &geom, fps, pts);
                ffmpeg_decoder = dec;
                proc_generator = gen;
                queued_frame = None;
                current_frame = None;
                let _ = execute!(out, crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                needs_render = true;
            }
        }

        // Check for end-of-file / loop
        if meta.duration_secs > 0.0 && current_pts >= meta.duration_secs {
            if is_looping {
                clock.seek_to(0.0);
                let (dec, gen) = start_decoders(&source, &geom, fps, 0.0);
                ffmpeg_decoder = dec;
                proc_generator = gen;
                queued_frame = None;
                current_frame = None;
                let _ = audio.start(0.0);
                continue;
            } else {
                return Ok(());
            }
        }

        // Receive and Synchronize Video Frames
        let receiver = if let Some(dec) = &ffmpeg_decoder {
            Some(&dec.frame_receiver)
        } else if let Some(gen) = &proc_generator {
            Some(&gen.frame_receiver)
        } else {
            None
        };

        if let Some(rx) = receiver {
            // Fill queued_frame if empty
            if queued_frame.is_none() {
                if let Ok(f) = rx.try_recv() {
                    queued_frame = Some(f);
                }
            }
        }

        if let Some(f) = queued_frame.take() {
            if f.pts_secs <= current_pts {
                // Time to display this frame!
                let mut best_frame = f;

                // Frame-skipping: catch up if falling behind clock
                if let Some(rx) = receiver {
                    while best_frame.pts_secs < current_pts - frame_interval * 1.5 {
                        if let Ok(next) = rx.try_recv() {
                            best_frame = next;
                        } else {
                            break;
                        }
                    }
                }

                current_frame = Some(best_frame);
                needs_render = true;
            } else {
                // Frame is in future, put it back
                queued_frame = Some(f);
            }
        }

        // 1. Process Input Events
        // Calculate dynamic wait time until next frame is due
        let wait_duration = if clock.is_paused() {
            Duration::from_millis(50)
        } else if let Some(next) = &queued_frame {
            let delay = (next.pts_secs - current_pts).max(0.001);
            Duration::from_secs_f64(delay.min(frame_interval))
        } else {
            Duration::from_millis(4)
        };

        if event::poll(wait_duration)? {
            while event::poll(Duration::from_millis(0))? {
                let ev = event::read()?;
                match ev {
                    Event::Key(key) if key.kind == KeyEventKind::Press => {
                        last_interaction = Instant::now();
                        needs_render = true;
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char(' ') | KeyCode::Char('k') => {
                                let paused = clock.toggle_pause();
                                ui_state.is_paused = paused;
                                if paused {
                                    audio.pause();
                                    ui_state.set_message("PAUSED");
                                } else {
                                    audio.resume();
                                    ui_state.set_message("PLAYING");
                                }
                            }
                            KeyCode::Right | KeyCode::Char('l') => {
                                let new_pts = (clock.get_pts() + 5.0).min(meta.duration_secs.max(60000.0));
                                clock.seek_to(new_pts);
                                let (dec, gen) = start_decoders(&source, &geom, fps, new_pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                let _ = audio.start(new_pts);
                                ui_state.set_message("+5s Seek");
                            }
                            KeyCode::Left | KeyCode::Char('j') => {
                                let new_pts = (clock.get_pts() - 5.0).max(0.0);
                                clock.seek_to(new_pts);
                                let (dec, gen) = start_decoders(&source, &geom, fps, new_pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                let _ = audio.start(new_pts);
                                ui_state.set_message("-5s Seek");
                            }
                            KeyCode::Char(']') | KeyCode::Char('>') => {
                                let new_pts = (clock.get_pts() + 15.0).min(meta.duration_secs.max(60000.0));
                                clock.seek_to(new_pts);
                                let (dec, gen) = start_decoders(&source, &geom, fps, new_pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                let _ = audio.start(new_pts);
                                ui_state.set_message("+15s Seek");
                            }
                            KeyCode::Char('[') | KeyCode::Char('<') => {
                                let new_pts = (clock.get_pts() - 15.0).max(0.0);
                                clock.seek_to(new_pts);
                                let (dec, gen) = start_decoders(&source, &geom, fps, new_pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                let _ = audio.start(new_pts);
                                ui_state.set_message("-15s Seek");
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() && meta.duration_secs > 0.0 => {
                                let frac = (c as u32 - '0' as u32) as f64 / 10.0;
                                let new_pts = meta.duration_secs * frac;
                                clock.seek_to(new_pts);
                                let (dec, gen) = start_decoders(&source, &geom, fps, new_pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                let _ = audio.start(new_pts);
                                ui_state.set_message(format!("Seek {:2.0}%", frac * 100.0));
                            }
                            KeyCode::Up | KeyCode::Char('+') | KeyCode::Char('=') => {
                                let new_vol = (audio.get_volume() + 0.10).min(3.0);
                                audio.set_volume(new_vol);
                                ui_state.volume = new_vol;
                                ui_state.set_message(format!("Volume {:3.0}%", new_vol * 100.0));
                            }
                            KeyCode::Down | KeyCode::Char('-') | KeyCode::Char('_') => {
                                let new_vol = (audio.get_volume() - 0.10).max(0.0);
                                audio.set_volume(new_vol);
                                ui_state.volume = new_vol;
                                ui_state.set_message(format!("Volume {:3.0}%", new_vol * 100.0));
                            }
                            KeyCode::Char('m') => {
                                let muted = audio.toggle_mute();
                                ui_state.is_muted = muted;
                                ui_state.set_message(if muted { "Muted" } else { "Unmuted" });
                            }
                            KeyCode::Char('v') | KeyCode::Char('M') => {
                                current_mode = current_mode.next();
                                ui_state.render_mode = current_mode;
                                let (px_c, px_r) = current_mode.subpixels();
                                geom = RenderGeometry::calculate(
                                    cols,
                                    rows.saturating_sub(1),
                                    meta.width,
                                    meta.height,
                                    current_scaling,
                                    px_c,
                                    px_r,
                                );
                                let pts = clock.get_pts();
                                let (dec, gen) = start_decoders(&source, &geom, fps, pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                current_frame = None;
                                let _ = execute!(out, crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                                ui_state.set_message(format!("Mode: {}", current_mode.name()));
                            }
                            KeyCode::Char('a') => {
                                current_scaling = current_scaling.next();
                                ui_state.scaling_mode = current_scaling;
                                let (px_c, px_r) = current_mode.subpixels();
                                geom = RenderGeometry::calculate(
                                    cols,
                                    rows.saturating_sub(1),
                                    meta.width,
                                    meta.height,
                                    current_scaling,
                                    px_c,
                                    px_r,
                                );
                                let pts = clock.get_pts();
                                let (dec, gen) = start_decoders(&source, &geom, fps, pts);
                                ffmpeg_decoder = dec;
                                proc_generator = gen;
                                queued_frame = None;
                                current_frame = None;
                                let _ = execute!(out, crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                                ui_state.set_message(format!("Aspect: {}", current_scaling.name()));
                            }
                            KeyCode::Char('f') => {
                                current_filter = current_filter.next();
                                ui_state.color_filter = current_filter;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message(format!("Filter: {}", current_filter.name()));
                            }
                            KeyCode::Char('b') => {
                                brightness = (brightness - 10).max(-100);
                                ui_state.brightness = brightness;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message(format!("Brightness: {}", brightness));
                            }
                            KeyCode::Char('B') => {
                                brightness = (brightness + 10).min(100);
                                ui_state.brightness = brightness;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message(format!("Brightness: +{}", brightness));
                            }
                            KeyCode::Char('c') => {
                                contrast = (contrast - 0.1).max(0.2);
                                ui_state.contrast = contrast;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message(format!("Contrast: {:.1}x", contrast));
                            }
                            KeyCode::Char('C') => {
                                contrast = (contrast + 0.1).min(3.0);
                                ui_state.contrast = contrast;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message(format!("Contrast: {:.1}x", contrast));
                            }
                            KeyCode::Char('i') => {
                                invert = !invert;
                                ui_state.invert = invert;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message(if invert { "Colors Inverted" } else { "Colors Normal" });
                            }
                            KeyCode::Char('r') => {
                                brightness = 0;
                                contrast = 1.0;
                                invert = false;
                                current_filter = ColorFilter::None;
                                ui_state.brightness = brightness;
                                ui_state.contrast = contrast;
                                ui_state.invert = invert;
                                ui_state.color_filter = current_filter;
                                lut = ColorLut::new(brightness, contrast, invert, current_filter);
                                ui_state.set_message("Adjustments Reset");
                            }
                            KeyCode::Char('p') | KeyCode::Char('L') => {
                                is_looping = !is_looping;
                                ui_state.is_looping = is_looping;
                                ui_state.set_message(if is_looping { "Looping On" } else { "Looping Off" });
                            }
                            KeyCode::Tab | KeyCode::Char('o') => {
                                osd_mode = osd_mode.next();
                                ui_state.osd_mode = osd_mode;
                                ui_state.set_message(match osd_mode {
                                    OsdMode::AutoHide => "OSD: Auto-hide",
                                    OsdMode::AlwaysOn => "OSD: Always Visible",
                                    OsdMode::Off => "OSD: Hidden",
                                });
                            }
                            KeyCode::Char('?') | KeyCode::Char('h') => {
                                ui_state.show_help = !ui_state.show_help;
                            }
                            _ => {}
                        }
                    }
                    Event::Resize(new_cols, new_rows) => {
                        cols = new_cols;
                        rows = new_rows;
                        let (px_c, px_r) = current_mode.subpixels();
                        geom = RenderGeometry::calculate(
                            cols,
                            rows.saturating_sub(1),
                            meta.width,
                            meta.height,
                            current_scaling,
                            px_c,
                            px_r,
                        );
                        let pts = clock.get_pts();
                        let (dec, gen) = start_decoders(&source, &geom, fps, pts);
                        ffmpeg_decoder = dec;
                        proc_generator = gen;
                        queued_frame = None;
                        current_frame = None;
                        let _ = execute!(out, crossterm::terminal::Clear(crossterm::terminal::ClearType::All));
                        needs_render = true;
                    }
                    _ => {}
                }
            }
        }

        // Render Frame to Buffer if updated
        if needs_render {
            if let Some(frame) = &current_frame {
                buffer.clear();
                buffer.push_str("\x1b[H"); // Cursor home

                match current_mode {
                    RenderMode::HalfBlock => render_half_block(frame, &geom, &lut, &mut buffer),
                    RenderMode::Quadrant => render_quadrant(frame, &geom, &lut, &mut buffer),
                    RenderMode::Braille => render_braille(frame, &geom, &lut, &mut buffer),
                    RenderMode::Shades | RenderMode::Ascii | RenderMode::Mono => {
                        render_ascii(frame, &geom, &lut, current_mode, &mut buffer)
                    }
                }

                // Determine OSD visibility
                let osd_visible = match osd_mode {
                    OsdMode::AlwaysOn => true,
                    OsdMode::Off => false,
                    OsdMode::AutoHide => {
                        last_interaction.elapsed().as_secs_f32() < 3.0 || ui_state.is_paused
                    }
                };

                render_ui(&ui_state, cols, rows, osd_visible, &mut buffer);

                // Write whole screen in single I/O burst
                out.write_all(buffer.as_bytes())?;
                out.flush()?;

                frame_count += 1;
                if fps_timer.elapsed() >= Duration::from_secs(1) {
                    let measured_fps = frame_count as f64 / fps_timer.elapsed().as_secs_f64();
                    ui_state.fps = measured_fps;
                    frame_count = 0;
                    fps_timer = Instant::now();
                }
            }
        }
    }
}
