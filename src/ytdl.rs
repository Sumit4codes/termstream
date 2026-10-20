use std::path::PathBuf;
use std::process::Command;
use anyhow::{bail, Context, Result};

pub const BAD_APPLE_URL: &str = "https://www.youtube.com/watch?v=FtutLA63Cp8";
pub const RICKROLL_URL: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

#[derive(Debug, Clone)]
pub struct ResolvedStream {
    pub title: String,
    pub video_url: String,
    pub audio_url: String,
}

pub struct YtDlp;

impl YtDlp {
    /// Check if a given string looks like a web URL or YouTube reference
    pub fn is_web_url(input: &str) -> bool {
        let trimmed = input.trim();
        trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("yt:")
            || trimmed.starts_with("ytdl:")
            || trimmed.starts_with("youtube:")
    }

    /// Normalize user input into a fetchable URL
    pub fn normalize_url(input: &str) -> String {
        let trimmed = input.trim();
        if let Some(id) = trimmed.strip_prefix("yt:") {
            if id.starts_with("http://") || id.starts_with("https://") {
                id.to_string()
            } else {
                format!("https://www.youtube.com/watch?v={}", id)
            }
        } else if let Some(id) = trimmed.strip_prefix("ytdl:") {
            id.to_string()
        } else if let Some(id) = trimmed.strip_prefix("youtube:") {
            if id.starts_with("http://") || id.starts_with("https://") {
                id.to_string()
            } else {
                format!("https://www.youtube.com/watch?v={}", id)
            }
        } else {
            trimmed.to_string()
        }
    }

    /// Locate yt-dlp binary on the system
    pub fn find_binary() -> Result<PathBuf> {
        // 1. Try standard PATH
        if let Ok(output) = Command::new("yt-dlp").arg("--version").output() {
            if output.status.success() {
                return Ok(PathBuf::from("yt-dlp"));
            }
        }

        // 2. Try ~/.local/bin/yt-dlp
        if let Ok(home) = std::env::var("HOME") {
            let local_bin = PathBuf::from(home).join(".local/bin/yt-dlp");
            if local_bin.exists() {
                return Ok(local_bin);
            }
        }

        // 3. Try /usr/local/bin/yt-dlp
        let usr_local = PathBuf::from("/usr/local/bin/yt-dlp");
        if usr_local.exists() {
            return Ok(usr_local);
        }

        bail!(
            "yt-dlp was not found on your system.\n\
             To stream YouTube or web videos, please install yt-dlp:\n\
             pip install --user yt-dlp   OR   sudo apt install yt-dlp"
        );
    }

    /// Resolve video and audio streaming URLs for a web link
    pub fn resolve(url: &str) -> Result<ResolvedStream> {
        let yt_binary = Self::find_binary()?;
        let target_url = Self::normalize_url(url);

        let mut cmd = Command::new(yt_binary);

        // Check for node, bun, or deno JS runtimes to prevent YouTube deprecation warnings
        if Command::new("node").arg("--version").output().is_ok() {
            cmd.args(["--js-runtimes", "node"]);
        } else if Command::new("bun").arg("--version").output().is_ok() {
            cmd.args(["--js-runtimes", "bun"]);
        } else if Command::new("deno").arg("--version").output().is_ok() {
            cmd.args(["--js-runtimes", "deno"]);
        }

        cmd.args([
            "--no-playlist",
            "--no-warnings",
            "--print", "%(title)s",
            "-g",
            "-f", "bestvideo[height<=720]+bestaudio/best[height<=720]/best",
            &target_url,
        ]);

        let output = cmd
            .output()
            .context("Failed to execute yt-dlp. Please ensure yt-dlp is functioning.")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            bail!("yt-dlp extraction failed: {}", stderr.trim());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = stdout.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();

        if lines.is_empty() {
            bail!("yt-dlp returned no stream URLs for: {}", target_url);
        }

        let title = lines[0].to_string();

        let (video_url, audio_url) = if lines.len() >= 3 {
            // Line 0: Title, Line 1: Video stream URL, Line 2: Audio stream URL
            (lines[1].to_string(), lines[2].to_string())
        } else if lines.len() >= 2 {
            // Line 0: Title, Line 1: Combined video+audio URL
            (lines[1].to_string(), lines[1].to_string())
        } else {
            // Single URL returned
            (lines[0].to_string(), lines[0].to_string())
        };

        Ok(ResolvedStream {
            title,
            video_url,
            audio_url,
        })
    }
}
