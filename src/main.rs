use anyhow::{bail, Context, Result};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

// -----------------------------------------------------------------------------
// CLI STYLING
// -----------------------------------------------------------------------------

const MY_STYLES: Styles = Styles::styled()
    .header(AnsiColor::Yellow.on_default().effects(Effects::BOLD))
    .usage(AnsiColor::Green.on_default().effects(Effects::BOLD))
    .literal(AnsiColor::Cyan.on_default());

// -----------------------------------------------------------------------------
// CLI
// -----------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "ff", version, about = "Practical ffmpeg wrapper", styles = MY_STYLES)]
struct Cli {
    /// Overwrite output files without asking (passes `-y` to ffmpeg).
    ///
    /// If not provided, ffmpeg's native interactive prompt is shown.
    #[arg(long, global = true)]
    force: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract audio track from a media file.
    #[command(
        override_usage = "ff extract <INPUT> [OUTPUT]",
        after_help = "\
Examples:
  ff extract video.mp4
  ff extract video.mp4 audio.mp3
"
    )]
    Extract {
        /// Input media file (video or audio/video container).
        input: String,

        /// Optional output MP3 file.
        ///
        /// Default: `<INPUT_BASENAME>.mp3`
        output: Option<String>,
    },

    /// Split into separate video-only and audio-only files.
    #[command(
        override_usage = "ff split <INPUT> [VIDEO_OUTPUT] [AUDIO_OUTPUT]",
        after_help = "\
Examples:
  ff split movie.mp4
  ff split movie.mp4 video.mp4 audio.mp3
"
    )]
    Split {
        /// Input media file.
        input: String,

        /// Optional video-only output.
        ///
        /// Default: `<INPUT_BASENAME>_split.<original extension>`
        video_output: Option<String>,

        /// Optional audio-only output.
        ///
        /// Default: `<INPUT_BASENAME>_split.mp3`
        audio_output: Option<String>,
    },

    /// Merge a video file and an audio file into one container.
    #[command(
        override_usage = "ff merge <VIDEO> <AUDIO> [OUTPUT]",
        after_help = "\
Examples:
  ff merge video.mp4 audio.m4a
  ff merge v.mp4 a.flac final.mp4
"
    )]
    Merge {
        /// Video stream source.
        video: String,

        /// Audio stream source.
        audio: String,

        /// Optional merged output file.
        ///
        /// Default: `<VIDEO_BASENAME>_merged.<video extension>`
        output: Option<String>,
    },

    /// Crop a time range from a media file.
    #[command(
        override_usage = "ff crop <INPUT> [OUTPUT] -s <START> -e <END> [--copy]",
        after_help = "\
By default this performs precise trimming (re-encoding).
Use --copy for fast keyframe-aligned trimming without re-encoding.

Examples:
  ff crop input.mp4 -s 00:01:00 -e 00:02:00
  ff crop input.mp4 out.mp4 -s 00:00:10 -e 00:00:20
  ff crop input.mp4 -s 00:01:00 -e 00:02:00 --copy
"
    )]
    Crop {
        /// Input media file.
        input: String,

        /// Optional output file.
        ///
        /// Default: `<INPUT_BASENAME>_cropped.<original extension>`
        output: Option<String>,

        /// Start timestamp (HH:MM:SS).
        #[arg(short, long)]
        start: String,

        /// End timestamp (HH:MM:SS).
        #[arg(short, long)]
        end: String,

        /// Fast mode (no re-encode, cuts only on keyframes).
        #[arg(long)]
        copy: bool,
    },

    /// Generate shell completions.
    #[command(
        override_usage = "ff completions <SHELL>",
        after_help = "\
For Git Bash use:
  ff completions bash > ~/.ff-complete.sh
  echo 'source ~/.ff-complete.sh' >> ~/.bashrc
"
    )]
    Completions {
        /// Target shell.
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

#[derive(ValueEnum, Clone)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
}

// -----------------------------------------------------------------------------
// MAIN
// -----------------------------------------------------------------------------

fn main() -> Result<()> {
    ensure_ffmpeg_installed()?;

    let cli = Cli::parse();

    match &cli.command {
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            generate(map_shell(shell), &mut cmd, "ff", &mut io::stdout());
            return Ok(());
        }
        _ => cli.command.execute(cli.force)?,
    }

    println!("✅ Done!");
    Ok(())
}

// -----------------------------------------------------------------------------
// EXECUTION
// -----------------------------------------------------------------------------

impl Commands {
    fn execute(&self, force: bool) -> Result<()> {
        match self {
            Commands::Extract { input, output } => {
                let output = output.clone().unwrap_or_else(|| replace_ext(input, "mp3"));

                Ffmpeg::new(force)
                    .args(["-i", input])
                    .args(["-vn", "-acodec", "libmp3lame", "-q:a", "2", &output])
                    .run()
            }

            Commands::Split {
                input,
                video_output,
                audio_output,
            } => {
                let video_out =
                    video_output.clone().unwrap_or_else(|| postfix_with_same_ext(input, "_split"));
                let audio_out =
                    audio_output.clone().unwrap_or_else(|| postfix_with_ext(input, "_split", "mp3"));

                Ffmpeg::new(force)
                    .args(["-i", input])
                    .args(["-c:v", "copy", "-an", &video_out])
                    .args(["-c:a", "libmp3lame", "-q:a", "2", "-vn", &audio_out])
                    .run()
            }

            Commands::Merge {
                video,
                audio,
                output,
            } => {
                let output =
                    output.clone().unwrap_or_else(|| postfix_with_same_ext(video, "_merged"));

                Ffmpeg::new(force)
                    .args(["-i", video])
                    .args(["-i", audio])
                    .args(["-c", "copy", "-map", "0:v:0", "-map", "1:a:0", &output])
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

                let output =
                    output.clone().unwrap_or_else(|| postfix_with_same_ext(input, "_cropped"));

                if *copy {
                    Ffmpeg::new(force)
                        .args(["-ss", start, "-to", end])
                        .args(["-i", input])
                        .args(["-c", "copy", "-avoid_negative_ts", "1", &output])
                        .run()
                } else {
                    Ffmpeg::new(force)
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
                            &output,
                        ])
                        .run()
                }
            }

            Commands::Completions { .. } => unreachable!(),
        }
    }
}

// -----------------------------------------------------------------------------
// FFMPEG BUILDER
// -----------------------------------------------------------------------------

struct Ffmpeg {
    args: Vec<String>,
}

impl Ffmpeg {
    fn new(force: bool) -> Self {
        let mut args = Vec::new();
        if force {
            args.push("-y".into());
        }
        Self { args }
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

        if status.success() {
            Ok(())
        } else {
            bail!("ffmpeg exited with an error.")
        }
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

fn validate_time(t: &str) -> Result<()> {
    if !t.contains(':') {
        bail!("Invalid time '{t}', expected HH:MM:SS.");
    }
    Ok(())
}

// -----------------------------------------------------------------------------
// FILENAME HELPERS
// -----------------------------------------------------------------------------

fn replace_ext(input: &str, new_ext: &str) -> String {
    Path::new(input)
        .with_extension(new_ext)
        .to_string_lossy()
        .into_owned()
}

fn postfix_with_same_ext(input: &str, postfix: &str) -> String {
    let path = Path::new(input);
    let stem = path.file_stem().unwrap().to_string_lossy();
    let ext = path.extension().unwrap_or_default().to_string_lossy();
    build_filename(path, &format!("{stem}{postfix}"), &ext)
}

fn postfix_with_ext(input: &str, postfix: &str, ext: &str) -> String {
    let path = Path::new(input);
    let stem = path.file_stem().unwrap().to_string_lossy();
    build_filename(path, &format!("{stem}{postfix}"), ext)
}

fn build_filename(base: &Path, stem: &str, ext: &str) -> String {
    let mut new = PathBuf::from(base.parent().unwrap_or(Path::new("")));
    new.push(format!("{stem}.{ext}"));
    new.to_string_lossy().into_owned()
}

// -----------------------------------------------------------------------------
// COMPLETION MAPPING
// -----------------------------------------------------------------------------

fn map_shell(shell: &CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => Shell::Fish,
        CompletionShell::PowerShell => Shell::PowerShell,
    }
}