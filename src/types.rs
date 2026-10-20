use clap::ValueEnum;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum RenderMode {
    /// 2 vertical pixels per character cell using unicode half blocks (▀) with 24-bit TrueColor
    HalfBlock,
    /// 2x4 dots per character cell using unicode Braille patterns with TrueColor
    Braille,
    /// Density shading using Unicode block characters ( ░▒▓█) with TrueColor
    Shades,
    /// Classic ASCII luminance ramp (@%#*+=-:. ) with TrueColor
    Ascii,
    /// Retro monochrome ASCII art (green/amber phosphor or gray)
    Mono,
}

impl RenderMode {
    pub fn subpixels(&self) -> (u32, u32) {
        match self {
            RenderMode::HalfBlock => (1, 2),
            RenderMode::Braille => (2, 4),
            RenderMode::Shades | RenderMode::Ascii | RenderMode::Mono => (1, 2),
        }
    }

    pub fn next(&self) -> Self {
        match self {
            RenderMode::HalfBlock => RenderMode::Braille,
            RenderMode::Braille => RenderMode::Shades,
            RenderMode::Shades => RenderMode::Ascii,
            RenderMode::Ascii => RenderMode::Mono,
            RenderMode::Mono => RenderMode::HalfBlock,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            RenderMode::HalfBlock => "Half-Block (TrueColor ▀)",
            RenderMode::Braille => "Braille 2x4 Matrix (⠓⠸)",
            RenderMode::Shades => "Unicode Shades ( ░▒▓█)",
            RenderMode::Ascii => "ASCII Ramp (@%#*+=-:. )",
            RenderMode::Mono => "Retro Mono Phosphor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ScalingMode {
    /// Preserve aspect ratio, letterboxing with black bars
    Fit,
    /// Crop edges to fill full terminal
    Fill,
    /// Stretch to terminal boundaries
    Stretch,
}

impl ScalingMode {
    pub fn next(&self) -> Self {
        match self {
            ScalingMode::Fit => ScalingMode::Fill,
            ScalingMode::Fill => ScalingMode::Stretch,
            ScalingMode::Stretch => ScalingMode::Fit,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ScalingMode::Fit => "Fit (Letterbox)",
            ScalingMode::Fill => "Fill (Crop)",
            ScalingMode::Stretch => "Stretch",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ColorFilter {
    None,
    MatrixGreen,
    AmberCrt,
    CyberCyan,
    Grayscale,
}

impl ColorFilter {
    pub fn next(&self) -> Self {
        match self {
            ColorFilter::None => ColorFilter::MatrixGreen,
            ColorFilter::MatrixGreen => ColorFilter::AmberCrt,
            ColorFilter::AmberCrt => ColorFilter::CyberCyan,
            ColorFilter::CyberCyan => ColorFilter::Grayscale,
            ColorFilter::Grayscale => ColorFilter::None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            ColorFilter::None => "Normal",
            ColorFilter::MatrixGreen => "Matrix Green CRT",
            ColorFilter::AmberCrt => "Amber CRT",
            ColorFilter::CyberCyan => "Cyberpunk Cyan",
            ColorFilter::Grayscale => "Grayscale",
        }
    }
}

#[derive(Debug, Clone)]
pub struct MediaMetadata {
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub duration_secs: f64,
    pub has_audio: bool,
    pub audio_sample_rate: u32,
    pub audio_channels: u16,
    pub title: Option<String>,
}

#[derive(Debug, Clone)]
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pts_secs: f64,
    pub data: Vec<u8>, // RGB24 layout: 3 bytes per pixel (R, G, B)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdMode {
    AutoHide, // Shows for 3 seconds after interaction
    AlwaysOn,
    Off,
}

impl OsdMode {
    pub fn next(&self) -> Self {
        match self {
            OsdMode::AutoHide => Self::AlwaysOn,
            OsdMode::AlwaysOn => Self::Off,
            OsdMode::Off => Self::AutoHide,
        }
    }
}
