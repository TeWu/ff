use anyhow::{bail, Context, Result};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{Parser, Subcommand};
use std::process::{Command, ExitStatus};

// -----------------------------------------------------------------------------
// CLI STYLING
// -----------------------------------------------------------------------------

const MY_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default());

// -----------------------------------------------------------------------------
// CLI DEFINITION
// -----------------------------------------------------------------------------

#[derive(Parser)]
#[command(
    name = "ff",
    bin_name = "ff",
    version,
    about = "Practical ffmpeg wrapper",
    styles = MY_STYLES
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Split a video into video-only and audio-only files
    Split {
        input: String,
        video_output: String,
        audio_output: String,
    },

    /// Extract audio from a video
    Extract {
        input: String,
        output: String,
    },

    /// Join separate video and audio into one file
    Join {
        video: String,
        audio: String,
        output: String,
    },

    /// Crop a section of media
    ///
    /// By default performs precise trimming (re-encoding).
    /// Use --copy for fast keyframe-aligned trimming.
    Crop {
        input: String,
        output: String,

        #[arg(short, long, help = "Start time (HH:MM:SS)")]
        start: String,

        #[arg(short, long, help = "End time (HH:MM:SS)")]
        end: String,

        #[arg(long, help = "Fast mode (no re-encode, keyframe aligned)")]
        copy: bool,
    },
}

// -----------------------------------------------------------------------------
// MAIN
// -----------------------------------------------------------------------------

fn main() -> Result<()> {
    ensure_ffmpeg_installed()?;

    let cli = Cli::parse();
    cli.command.execute()?;

    println!("✅ Done!");
    Ok(())
}

// -----------------------------------------------------------------------------
// COMMAND EXECUTION
// -----------------------------------------------------------------------------

impl Commands {
    fn execute(&self) -> Result<()> {
        match self {
            Commands::Split {
                input,
                video_output,
                audio_output,
            } => {
                println!("🪓 Splitting '{input}'...");
                Ffmpeg::new()
                    .args(["-i", input])
                    .args(["-c:v", "copy", "-an", video_output])
                    .args(["-c:a", "libmp3lame", "-q:a", "2", "-vn", audio_output])
                    .run()
            }

            Commands::Extract { input, output } => {
                println!("🎵 Extracting audio...");
                Ffmpeg::new()
                    .args(["-i", input])
                    .args(["-vn", "-acodec", "libmp3lame", "-q:a", "2", output])
                    .run()
            }

            Commands::Join {
                video,
                audio,
                output,
            } => {
                println!("🔗 Joining streams...");
                Ffmpeg::new()
                    .args(["-i", video])
                    .args(["-i", audio])
                    .args(["-c", "copy", "-map", "0:v:0", "-map", "1:a:0", output])
                    .run()
            }

            Commands::Crop {
                input,
                output,
                start,
                end,
                copy,
            } => {
                validate_time(start)?;
                validate_time(end)?;

                if *copy {
                    println!("⚡ Fast crop (stream copy) {start} → {end}");
                    // Fast seek MUST place -ss before input
                    Ffmpeg::new()
                        .args(["-ss", start, "-to", end])
                        .args(["-i", input])
                        .args(["-c", "copy", "-avoid_negative_ts", "1", output])
                        .run()
                } else {
                    println!("🎯 Precise crop (re-encode) {start} → {end}");
                    // Accurate seek MUST place -ss after input
                    Ffmpeg::new()
                        .args(["-i", input])
                        .args(["-ss", start, "-to", end])
                        .args([
                            "-c:v",
                            "libx264",
                            "-preset",
                            "slow",
                            "-crf",
                            "23",
                            "-c:a",
                            "aac",
                            "-movflags",
                            "+faststart",
                            output,
                        ])
                        .run()
                }
            }
        }
    }
}

// -----------------------------------------------------------------------------
// FFMPEG BUILDER (avoids &[&str] soup)
// -----------------------------------------------------------------------------

struct Ffmpeg {
    args: Vec<String>,
}

impl Ffmpeg {
    fn new() -> Self {
        Self { args: Vec::new() }
    }

    fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    fn args<I, S>(mut self, args: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    fn run(self) -> Result<()> {
        let status = Command::new("ffmpeg")
            .args(&self.args)
            .status()
            .context("Failed to start ffmpeg")?;

        ensure_success(status)
    }
}

// -----------------------------------------------------------------------------
// HELPERS
// -----------------------------------------------------------------------------

fn ensure_ffmpeg_installed() -> Result<()> {
    if Command::new("ffmpeg").arg("-version").output().is_err() {
        bail!("ffmpeg not found in PATH.");
    }
    Ok(())
}

fn ensure_success(status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("ffmpeg exited with an error.");
    }
}

/// Minimal validation to catch obvious mistakes early.
fn validate_time(t: &str) -> Result<()> {
    if !t.contains(':') {
        bail!("Invalid time '{t}'. Expected HH:MM:SS format.");
    }
    Ok(())
}