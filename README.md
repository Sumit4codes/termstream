# TermStream 🎬📺

A high-performance, feature-complete terminal video player built from scratch in Rust with real-time audio/video synchronization, 24-bit TrueColor rendering, and multiple display modes.

---

## ✨ Features

- **Blazing Fast Native Rendering**:
  - **Half-Block Mode (`▀`)**: Displays 2 vertical pixels per character cell using ANSI 24-bit TrueColor foreground and background. Doubles the vertical resolution with square pixel aspect ratios.
  - **Braille 2x4 Matrix (`⠋⠙⠸`)**: 8 dots per cell (2 horizontal × 4 vertical dots) using Unicode Braille with 2x4 Bayer matrix dithering and TrueColor tinting for ultra-high-resolution matrix rendering.
  - **Unicode Block Shades (` ░▒▓█`)**: High-contrast textured luminance shading.
  - **ASCII Density Ramp (`@%#*+=-:. `)**: Classic retro terminal aesthetic.
  - **Retro Phosphor CRT Filters**: Matrix Green CRT, Amber CRT, Cyberpunk Cyan, and Monochrome Grayscale.

- **Audio-Video Synchronization (A/V Sync)**:
  - Monotonic master clock with automatic drift compensation and dynamic frame-skipping when terminal I/O latency occurs.
  - Real audio output powered by `rodio` / ALSA / PipeWire / PulseAudio.
  - Volume control (0%–150%) and instant mute toggle.

- **Interactive Terminal UI (OSD)**:
  - Bottom status bar showing playback state (`▶ PLAY` / `⏸ PAUSE`), elapsed & total time, visual progress bar, volume level, real-time FPS counter, and active render mode badge.
  - OSD modes: Auto-hide (fades out after 3 seconds of inactivity), Always On, or Hidden.
  - In-terminal Help HUD popup (`?` or `h`) displaying all keyboard shortcuts.

- **Real-Time Video Adjustments**:
  - On-the-fly Brightness (`b` / `B`) and Contrast (`c` / `C`) adjustments using precomputed 256-byte LUTs for zero per-pixel CPU penalty.
  - Color inversion toggle (`i`).
  - Aspect ratio modes (`a`): Fit (letterbox), Fill (crop), and Stretch.
  - Looping mode (`p` / `L`).

- **Universal Media Support**:
  - Local video files (`.mp4`, `.mkv`, `.webm`, `.avi`, `.mov`, `.gif`, etc.)
  - Network streams & URLs (HTTP/HTTPS, RTSP)
  - V4L2 Webcams (`/dev/video0`)
  - **Built-in Procedural Generators**: Demo modes playable immediately without any video file:
    - `test:plasma`: 90s demoscene psychedelic sine wave plasma
    - `test:matrix`: Falling digital rain
    - `test:cube`: 3D rotating wireframe/colored cube
    - `test:bars`: SMPTE color bars with moving sync pip

---

## 🚀 Quickstart

### Prerequisites

- `cargo` (Rust 1.75+)
- `ffmpeg` and `ffprobe` installed on your system:
  ```bash
  sudo apt install ffmpeg
  ```

### Build & Install

```bash
# Clone and enter the repository
cd termstream

# Build optimized release binary
cargo build --release

# Run binary directly
./target/release/termstream --test
```

Or install it globally to your `~/.cargo/bin`:
```bash
cargo install --path .
termstream --test
```

---

## 🎮 Usage Examples

```bash
# Play local video file
termstream /path/to/movie.mp4

# Play in Braille matrix mode
termstream -m braille /path/to/movie.mp4

# Play in retro Matrix Green CRT phosphor mode
termstream -m mono -f matrix-green /path/to/movie.mp4

# Play with video looped and no audio
termstream --loop --no-audio /path/to/animation.gif

# Play built-in 3D rotating cube demo
termstream test:cube

# Play built-in falling Matrix rain
termstream test:matrix

# Inspect media metadata without playing
termstream --info /path/to/movie.mp4
```

---

## ⌨️ Interactive Keybindings

| Key | Action |
|:---:|:---|
| <kbd>Space</kbd> / <kbd>k</kbd> | Toggle Play / Pause |
| <kbd>→</kbd> / <kbd>l</kbd> | Seek forward 5 seconds |
| <kbd>←</kbd> / <kbd>j</kbd> | Seek backward 5 seconds |
| <kbd>]</kbd> / <kbd>&gt;</kbd> | Seek forward 15 seconds |
| <kbd>[</kbd> / <kbd>&lt;</kbd> | Seek backward 15 seconds |
| <kbd>0</kbd> .. <kbd>9</kbd> | Seek directly to 0% .. 90% of video duration |
| <kbd>↑</kbd> / <kbd>+</kbd> | Volume up 5% |
| <kbd>↓</kbd> / <kbd>-</kbd> | Volume down 5% |
| <kbd>m</kbd> | Toggle Mute |
| <kbd>v</kbd> / <kbd>M</kbd> | Cycle Render Mode (Half-Block → Braille → Shades → ASCII → Mono) |
| <kbd>a</kbd> | Cycle Aspect Ratio (Fit → Fill → Stretch) |
| <kbd>f</kbd> | Cycle Color Filter (Normal → Matrix Green → Amber CRT → Cyan → Grayscale) |
| <kbd>b</kbd> / <kbd>B</kbd> | Decrease / Increase Brightness |
| <kbd>c</kbd> / <kbd>C</kbd> | Decrease / Increase Contrast |
| <kbd>i</kbd> | Invert Colors |
| <kbd>r</kbd> | Reset all picture adjustments |
| <kbd>p</kbd> / <kbd>L</kbd> | Toggle Looping |
| <kbd>Tab</kbd> / <kbd>o</kbd> | Cycle OSD Mode (Auto-hide → Always On → Off) |
| <kbd>?</kbd> / <kbd>h</kbd> | Toggle On-Screen Help Overlay |
| <kbd>q</kbd> / <kbd>Esc</kbd> | Quit Player |

---

## 📐 Architecture

TermStream uses a decoupled multi-threaded streaming architecture designed for zero latency and minimal CPU overhead:

```
┌─────────────────┐       Raw RGB24 Pipes
│ FFmpeg / Prober │ ──────────────────────────┐
└─────────────────┘                           │
                                              ▼
┌─────────────────┐  PCM S16LE  ┌────────────────────────┐
│  Rodio / Audio  │ ◄────────── │ Video & Audio Decoders │
└─────────────────┘             └────────────────────────┘
                                              │
                                              ▼ (Bounded Channel)
┌─────────────────┐    Ticks    ┌────────────────────────┐
│ Monotonic Clock │ ──────────► │ AV Sync & Frame Dropper│
└─────────────────┘             └────────────────────────┘
                                              │
                                              ▼
                                ┌────────────────────────┐
                                │ Color LUT & Geometries │
                                └────────────────────────┘
                                              │
                                              ▼
                                ┌────────────────────────┐
                                │ Mode Specific Renderer │
                                │ (Block/Braille/ASCII)  │
                                └────────────────────────┘
                                              │
                                              ▼
                                ┌────────────────────────┐
                                │ ANSI Escape Compressor │
                                └────────────────────────┘
                                              │
                                              ▼
                                ┌────────────────────────┐
                                │ Single Flush to Stdout │
                                └────────────────────────┘
```

1. **Decoder Pipeline**: Spawns an FFmpeg worker that pipes raw `rgb24` frames directly into memory, leveraging hardware SIMD `libswscale` to scale to the terminal's native cell dimensions.
2. **ANSI Code Compression**: In Half-Block and Braille modes, consecutive cells with identical foreground or background colors omit redundant ANSI color escape codes, saving up to 60% of terminal I/O bandwidth.
3. **Double Buffering & Flicker-free Writes**: Terminal output is pre-allocated and written in a single `stdout.write_all()` operation with the cursor homed (`\x1b[H`), completely preventing screen tearing and flickering.
4. **Crash-Resilient Terminal Restoration**: A terminal guard structure implements `Drop`, ensuring that raw mode is disabled and the alternate screen buffer is cleanly restored even on aborts or unexpected signals.

---

## 🧪 Running Tests

To run the unit and integration test suite:

```bash
cargo test
```
