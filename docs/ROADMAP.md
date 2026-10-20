# TermStream Popularity & Open Source Growth Roadmap 🚀

This document outlines the strategic roadmap to grow **TermStream** into a high-visibility, widely adopted terminal media tool in the Rust and CLI ecosystems.

---

## 🎯 Phase 1: High-Virality Feature Additions

- [x] **High-Resolution Quadrant Mode (2x2)**: Doubled horizontal TrueColor resolution without dither noise.
- [x] **Adaptive Edge-Preserving Braille Mode (2x4)**: High-density 8-dot matrix with contrast-aware dynamic thresholding.
- [x] **Dynamic Real-time Resolution Scaling**: Instant font zoom (`Ctrl + -`) and resize detection.
- [x] **Native YouTube & Web Streaming (`yt-dlp` integration)**:
  - Play any YouTube URL directly: `termstream "https://www.youtube.com/watch?v=..."`
  - Shorthand support: `termstream yt:dQw4w9WgXcQ`
  - Zero local disk writes; stream directly through FFmpeg pipe.
- [x] **One-Click Demos & Easter Eggs**:
  - `termstream --bad-apple`: Stream the classic *Bad Apple!!* benchmark in high-res Braille/Quadrant TrueColor.
  - `termstream --rickroll`: Instant smooth playback demo.
- [x] **Modern Terminal Graphics Protocols**:
  - Kitty Graphics Protocol (`-m kitty`) for true 1080p pixel rendering (Kitty, Ghostty, WezTerm).
  - Sixel protocol (`-m sixel`) support for iTerm2, WezTerm, foot, and mlterm.
- [x] **Storyboard & File Manager Preview**:
  - `--grid` / `--storyboard` generating a 3x3 keyframe thumbnail contact sheet in the terminal.
  - Integration plugin for `yazi`, `ranger`, and `lf`.

---

## 🎨 Phase 2: Visual Packaging & "5-Second Rule" README

- [ ] **Hero Media**:
  - High-framerate animated WebM / GIF demonstrating:
    - Side-by-side comparison of Half-Block, Quadrant, and Braille modes.
    - Phosphor CRT filters (Matrix Green, Amber, Cyberpunk Cyan).
    - Audio visualization / synchronized interactive OSD.
- [x] **README Badges & Architecture Diagram**:
  - CI status, license, and platform compatibility badges.
  - Comprehensive comparison table against `mpv --vo=caca`, `tplay`, and `chafa`.

---

## 📦 Phase 3: Zero-Friction Distribution

- [ ] **Crates.io Release**:
  - `cargo publish` for instant `cargo install termstream`.
- [x] **Automated GitHub Releases**:
  - GitHub Actions CI matrix with automated testing across Linux, macOS, and Windows.
  - Cross-compilation release pipeline for Linux (x86_64, aarch64), macOS (Intel, Apple Silicon), and Windows.
- [ ] **Package Repositories**:
  - Homebrew tap (`brew install sumit/tap/termstream`).
  - Arch Linux AUR (`termstream-bin`).

---

## 📢 Phase 4: Multi-Channel Launch Strategy

1. **Reddit `r/unixporn`**:
   - Focus: Aesthetic terminal setup with Matrix CRT green filter playing in a transparent terminal.
2. **Reddit `r/rust`**:
   - Focus: Deep-dive technical write-up on zero-allocation ANSI compression, A/V drift sync, and crossbeam concurrency.
3. **Hacker News (`Show HN`)**:
   - Post on Tuesday/Wednesday morning (6:00 AM - 9:00 AM PT).
4. **Social & Video Platforms**:
   - 15-second high-framerate clips on Twitter/X with `#rustlang`, `#linux`, `#cli`.
