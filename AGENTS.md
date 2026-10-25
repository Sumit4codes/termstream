# AGENTS.md

## Overview & Architecture

`termstream` is a standalone Rust CLI/library video player for terminals with TrueColor rendering, monotonic A/V sync, procedural patterns, and YouTube streaming.

- **Pipeline**:
  - `src/main.rs`: CLI parsing (`clap`), terminal lifecycle (`crossterm` raw mode + alternate screen), event loop, dynamic resize handling, and atomic frame flushing (`\x1b[?2026h` sync update).
  - `src/media.rs`: Probes media via `ffprobe` (`MediaProbe`) and streams raw `rgb24` frames from `ffmpeg` over stdout pipe into a bounded `crossbeam_channel::bounded(4)` (`VideoStreamDecoder`).
  - `src/audio.rs`: Spawns `ffmpeg` for raw PCM S16LE (44.1kHz stereo) consumed by `rodio` via non-blocking `PcmSource`. Applies +2.6 dB software preamp gain (`AudioPlayer::PREAMP_GAIN = 1.35`).
  - `src/clock.rs`: Monotonic clock tracking PTS; `main.rs` drops lagging video frames when rendering falls behind.
  - `src/render/`: Renders `VideoFrame` data to an in-memory `String` buffer. Includes `half_block`, `quadrant` (2x2), `braille` (2x4 with adaptive contrast thresholding), `ascii`/`shades`, `kitty` (Kitty Graphics Protocol chunked base64), and `sixel` (indexed 64-color RLE). Consecutive cells with matching colors omit redundant ANSI color codes.
  - `src/storyboard.rs`: Scoped multi-threaded 3x3 keyframe contact sheet generator (`--grid`).
  - `src/generator.rs`: Procedural patterns (`test:plasma`, `test:matrix`, `test:cube`, `test:bars`) requiring zero external binaries or files.
  - `src/ytdl.rs`: Discovers system `yt-dlp` (PATH, `~/.local/bin`, `/usr/local/bin`) and queries direct streaming URLs.

## System Prerequisites

- **Build time (Linux)**: ALSA development headers required for `rodio`/`cpal`:
  ```bash
  sudo apt-get install -y libasound2-dev ffmpeg
  ```
- **Runtime**:
  - `ffmpeg` and `ffprobe` are required for local files and web streams.
  - `yt-dlp` is optional, required only for YouTube/web URLs (`--bad-apple`, `--rickroll`, URLs).

## Build & Verification Commands

- **Check compilation**:
  ```bash
  cargo check
  ```
- **Run all tests**:
  ```bash
  cargo test
  ```
- **Run integration tests specifically**:
  ```bash
  cargo test --test player_tests
  ```
- **Run a single test**:
  ```bash
  cargo test --test player_tests <test_name> -- --exact
  # Example:
  cargo test --test player_tests test_render_kitty_output -- --exact
  ```
- **CI commands**: CI only executes `cargo check --verbose` and `cargo test --verbose` on Linux, macOS, and Windows.
- **Clippy / Fmt caveat**: CI does **not** enforce `cargo fmt --check` or `cargo clippy -D warnings`. Existing codebase has clippy warnings and unformatted files; do not run repo-wide `cargo fmt` or introduce strict `-D warnings` checks without scoped edits.

## Running Locally for Dev / Debug

Test playback without any media files or `ffmpeg` installed:
```bash
cargo run -- --test               # Default plasma pattern
cargo run -- test:matrix          # Matrix digital rain
cargo run -- test:cube            # 3D rotating cube
cargo run -- test:bars            # SMPTE color bars
cargo run -- --grid test:bars     # 3x3 storyboard output
```

Inspect media metadata without launching full terminal UI:
```bash
cargo run -- --info <file_or_url>
```

## Implementation Gotchas & Conventions

- **Terminal Cleanup**: Always ensure `TerminalCleanup` or proper `Drop` runs when modifying terminal setup in `main.rs`. Terminal enters raw mode, hides cursor, and enters alternate screen. On exit, it also sends `\x1b_Ga=d,d=a\x1b\` to clear any lingering Kitty graphics textures from GPU memory.
- **UTF-8 Slicing**: When truncating strings for terminal OSD or storyboard, never slice `&str` directly by byte indices. Use `termstream::storyboard::safe_truncate` (or `chars().take(...)`) to prevent panics on multibyte UTF-8 / CJK / emoji boundaries.
- **Audio Testing**: Unit tests should instantiate `AudioPlayer::new(..., false)` (disabled mode) or use `PcmSource` directly so tests do not fail on headless CI without audio devices.
- **Commit Style**: Use Conventional Commits (e.g. `feat(...)`, `fix(...)`, `perf(...)`).
