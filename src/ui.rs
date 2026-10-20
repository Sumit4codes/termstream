use std::fmt::Write;
use crate::types::{ColorFilter, OsdMode, RenderMode, ScalingMode};

pub struct UiState {
    pub current_pts: f64,
    pub duration: f64,
    pub is_paused: bool,
    pub volume: f32,
    pub is_muted: bool,
    pub is_looping: bool,
    pub render_mode: RenderMode,
    pub scaling_mode: ScalingMode,
    pub color_filter: ColorFilter,
    pub brightness: i32,
    pub contrast: f32,
    pub invert: bool,
    pub fps: f64,
    pub show_help: bool,
    pub osd_mode: OsdMode,
    pub message: Option<(String, std::time::Instant)>,
}

impl UiState {
    pub fn set_message(&mut self, msg: impl Into<String>) {
        self.message = Some((msg.into(), std::time::Instant::now()));
    }
}

pub fn format_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let hours = s / 3600;
    let mins = (s % 3600) / 60;
    let rem_s = s % 60;

    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, rem_s)
    } else {
        format!("{:02}:{:02}", mins, rem_s)
    }
}

pub fn render_ui(
    state: &UiState,
    cols: u16,
    rows: u16,
    osd_visible: bool,
    buffer: &mut String,
) {
    if rows < 4 || cols < 30 {
        return;
    }

    // Render Help HUD modal if active
    if state.show_help {
        render_help_modal(cols, rows, buffer);
        return;
    }

    if !osd_visible {
        return;
    }

    // Bottom status bar
    let bar_row = rows;
    let _ = write!(buffer, "\x1b[{};1H\x1b[48;2;18;20;26m\x1b[38;2;220;225;235m", bar_row);

    // Play/Pause icon
    let play_icon = if state.is_paused {
        "\x1b[38;2;250;180;50m ⏸ PAUSE "
    } else {
        "\x1b[38;2;80;220;120m ▶ PLAY "
    };

    // Time formatted
    let time_str = format!(
        "\x1b[38;2;240;240;240m{} / {}",
        format_time(state.current_pts),
        if state.duration > 0.0 {
            format_time(state.duration)
        } else {
            "--:--".to_string()
        }
    );

    // Render transient message or progress bar
    let mut middle_str = String::new();
    if let Some((msg, instant)) = &state.message {
        if instant.elapsed().as_secs_f32() < 2.5 {
            let _ = write!(middle_str, " \x1b[38;2;100;200;255m[ {} ] ", msg);
        }
    }

    if middle_str.is_empty() {
        // Progress bar
        let progress = if state.duration > 0.0 {
            (state.current_pts / state.duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let bar_width = (cols as usize).saturating_sub(65).clamp(10, 40);
        let filled = (progress * bar_width as f64).round() as usize;

        middle_str.push_str(" \x1b[38;2;120;140;180m[");
        for i in 0..bar_width {
            if i < filled {
                middle_str.push_str("\x1b[38;2;80;180;255m━");
            } else if i == filled {
                middle_str.push_str("\x1b[38;2;255;255;255m●");
            } else {
                middle_str.push_str("\x1b[38;2;70;75;90m─");
            }
        }
        middle_str.push_str("\x1b[38;2;120;140;180m] ");
    }

    // Volume badge
    let vol_str = if state.is_muted {
        "\x1b[38;2;240;90;90m🔇 MUTE ".to_string()
    } else {
        format!("\x1b[38;2;180;200;220m🔊 {:3.0}% ", (state.volume * 100.0).round())
    };

    // Mode badges
    let mode_badge = match state.render_mode {
        RenderMode::HalfBlock => "\x1b[38;2;100;220;255mBLOCK",
        RenderMode::Quadrant => "\x1b[38;2;120;255;200mQUAD",
        RenderMode::Braille => "\x1b[38;2;180;120;255mBRAILLE",
        RenderMode::Shades => "\x1b[38;2;255;160;100mSHADE",
        RenderMode::Ascii => "\x1b[38;2;140;255;140mASCII",
        RenderMode::Mono => "\x1b[38;2;80;255;80mMONO",
    };

    // Loop badge
    let loop_badge = if state.is_looping {
        "\x1b[38;2;100;240;150m🔁 "
    } else {
        ""
    };

    // FPS badge
    let fps_badge = format!("\x1b[38;2;150;160;180m{:4.1}fps ", state.fps);

    // Assemble text
    let left = format!("{}{} {}", play_icon, time_str, middle_str);
    let right = format!("{}{} {}[ {} ]  \x1b[38;2;110;120;140m(? for help) ", loop_badge, vol_str, fps_badge, mode_badge);

    // Calculate visible lengths for padding
    let left_clean = strip_ansi(&left);
    let right_clean = strip_ansi(&right);
    let total_len = left_clean.len() + right_clean.len();

    buffer.push_str(&left);
    if (cols as usize) > total_len {
        let pad = (cols as usize) - total_len;
        for _ in 0..pad {
            buffer.push(' ');
        }
    }
    buffer.push_str(&right);
    buffer.push_str("\x1b[0m");
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' {
                in_escape = false;
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn render_help_modal(cols: u16, rows: u16, buffer: &mut String) {
    let lines = [
        "─────────────── TermStream Keybindings ───────────────",
        "  Space            Play / Pause",
        "  Left / Right     Seek -/+ 5 seconds",
        "  [ / ]            Seek -/+ 15 seconds",
        "  0 .. 9           Seek to 0% .. 90%",
        "  Up / Down        Volume Up / Down (10%)",
        "  m                Toggle Mute",
        "  M                Cycle Render Mode (Block/Braille/ASCII)",
        "  a                Cycle Aspect Ratio (Fit/Fill/Stretch)",
        "  f                Cycle Color Filter (Green/Amber/Cyan/Mono)",
        "  b / B            Adjust Brightness (- / +)",
        "  c / C            Adjust Contrast (- / +)",
        "  i                Invert Colors",
        "  r                Reset Video Adjustments",
        "  l                Toggle Looping",
        "  Tab / o          Cycle OSD Mode (Auto / Always / Off)",
        "  ? / h            Toggle this Help Screen",
        "  q / Esc          Quit",
        "───────────────────────────────────────────────────────",
    ];

    let box_width = 57usize;
    let box_height = lines.len() + 2;

    if (cols as usize) < box_width || (rows as usize) < box_height {
        return;
    }

    let start_row = ((rows as usize).saturating_sub(box_height)) / 2 + 1;
    let start_col = ((cols as usize).saturating_sub(box_width)) / 2 + 1;

    // Top border
    let _ = write!(
        buffer,
        "\x1b[{};{}H\x1b[48;2;25;28;38m\x1b[38;2;120;160;255m┌",
        start_row, start_col
    );
    for _ in 0..(box_width - 2) {
        buffer.push('─');
    }
    buffer.push_str("┐");

    // Content lines
    for (i, line) in lines.iter().enumerate() {
        let r = start_row + 1 + i;
        let _ = write!(
            buffer,
            "\x1b[{};{}H\x1b[48;2;25;28;38m\x1b[38;2;120;160;255m│\x1b[38;2;230;235;245m",
            r, start_col
        );

        let line_len = line.chars().count();
        buffer.push_str(line);
        if line_len < box_width - 2 {
            for _ in 0..(box_width - 2 - line_len) {
                buffer.push(' ');
            }
        }
        buffer.push_str("\x1b[38;2;120;160;255m│");
    }

    // Bottom border
    let bot_row = start_row + box_height - 1;
    let _ = write!(
        buffer,
        "\x1b[{};{}H\x1b[48;2;25;28;38m\x1b[38;2;120;160;255m└",
        bot_row, start_col
    );
    for _ in 0..(box_width - 2) {
        buffer.push('─');
    }
    buffer.push_str("┘\x1b[0m");
}
