use anyhow::{bail, Context, Result};
use clap::builder::styling::{AnsiColor, Effects, Styles};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::{generate, Shell};
use std::cmp::Reverse;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
#[command(
    name = "ff",
    bin_name = "ff",
    version,
    about = "Practical ffmpeg wrapper",
    styles = MY_STYLES
)]
struct Cli {
    /// Overwrite output files without asking (passes `-y` to ffmpeg).
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
        verbatim_doc_comment,
        override_usage = "ff extract <INPUT> [OUTPUT]",
        after_help = "\
Examples:
  ff extract video.mp4
  ff extract video.mp4 audio.mp3
"
    )]
    Extract {
        #[arg(help = "Input media file (video or audio/video container).")]
        input: String,

        #[arg(help = "Output audio file.\n  Default: `<INPUT_BASENAME>.mp3`.")]
        output: Option<String>,
    },

    /// Convert a media file to a different format.
    #[command(
        verbatim_doc_comment,
        override_usage = "ff convert <INPUT> <OUTPUT>",
        after_help = "\
Format is determined by the output file extension.

Examples:
  ff convert input.flac output.mp3
  ff convert input.mov output.mp4
  ff convert input.mp4 output.webm
"
    )]
    Convert {
        #[arg(help = "Input file.")]
        input: String,

        #[arg(help = "Output file (extension determines format).")]
        output: String,
    },

    /// Split into separate video-only and audio-only files.
    #[command(
        verbatim_doc_comment,
        override_usage = "ff split <INPUT> [VIDEO_OUTPUT] [AUDIO_OUTPUT]",
        after_help = "\
Examples:
  ff split movie.mp4
  ff split movie.mp4 video.mp4 audio.mp3
"
    )]
    Split {
        #[arg(help = "Input video file.")]
        input: String,

        #[arg(help = "Video-only output.\n  Default: `<INPUT_BASENAME>_split.<original extension>`.")]
        video_output: Option<String>,

        #[arg(help = "Audio-only output.\n  Default: `<INPUT_BASENAME>_split.mp3`.")]
        audio_output: Option<String>,
    },

    /// Merge a video file and an audio file into one container.
    #[command(
        verbatim_doc_comment,
        override_usage = "ff merge <VIDEO> <AUDIO> [OUTPUT]",
        after_help = "\
Examples:
  ff merge video.mp4 audio.m4a
  ff merge v.mp4 a.flac final.mp4
"
    )]
    Merge {
        #[arg(help = "Video stream source.")]
        video: String,

        #[arg(help = "Audio stream source.")]
        audio: String,

        #[arg(help = "Merged output file.\n  Default: `<VIDEO_BASENAME>_merged.<video extension>`.")]
        output: Option<String>,
    },

    /// Crop a time range from a media file.
    #[command(
        verbatim_doc_comment,
        override_usage = "ff crop <INPUT> [OUTPUT] -s <START> -e <END> [--copy]",
        after_help = "\
By default this performs precise trimming (re-encoding).
Use --copy for fast keyframe-aligned trimming without re-encoding.

Examples:
  ff crop input.mp4 -e 00:02:00
  ff crop input.mp4 out.mp4 -s 00:00:10 -e 00:00:20
  ff crop input.mp4 -s 00:01:00 -e 00:02:00 --copy
"
    )]
    Crop {
        #[arg(help = "Input media file.")]
        input: String,

        #[arg(help = "Output media file.\n  Default: `<INPUT_BASENAME>_cropped.<original extension>`.")]
        output: Option<String>,

        #[arg(help = "Start timestamp (HH:MM:SS).\n  Default: beginning of file.")]
        #[arg(short, long)]
        start: Option<String>,

        #[arg(help = "End timestamp (HH:MM:SS).\n  Default: end of file.")]
        #[arg(short, long)]
        end: Option<String>,

        #[arg(help = "Fast mode (no re-encode, cuts only on keyframes).")]
        #[arg(long)]
        copy: bool,
    },

    /// Increase volume using dynamic normalization or percentile-based limiting.
    #[command(
        verbatim_doc_comment,
        override_usage = "ff loud <MODE> [OPTIONS] <INPUT> [OUTPUT]",
        after_help = "\
Modes:
  dyn   Dynamic normalization - quiets are louder, peaks are tamed.
          Does NOT preserve original dynamics. Best for speech/podcasts.
          Use -I to control target integrated loudness in LUFS.
  lim   Applies a steady boost, so that <PERCENT>% of samples hit the limiter.
          Keeps dynamics intact. Best for music.
          Use --top to set the percentile.

Examples:
  ff loud dyn speech.mp3
  ff loud dyn -I -9 speech.mp3
  ff loud lim --top 5 music.mp3
  ff loud lim --top 20 music.mp3 out.mp3"
    )]
    Loud {
        #[arg(value_enum)]
        mode: LoudMode,
        input: String,
        output: Option<String>,
        #[arg(long = "top", default_value = "0")]
        percent: f64,
        #[arg(short = 'I', long = "integrated", default_value = "-14",
          help = "Target integrated loudness in LUFS (dyn mode only). Range: -70 to -5.")]
        integrated: f64,
    },

    /// Generate shell completions.
    #[command(
        verbatim_doc_comment,
        override_usage = "ff completions <SHELL>",
        after_help = "\
For Git Bash use:
  ff completions bash > ~/.ff-complete.sh
  echo 'source ~/.ff-complete.sh' >> ~/.bashrc
"
    )]
    Completions {
        #[arg(help = "Target shell.")]
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

#[derive(ValueEnum, Clone)]
enum LoudMode { Dyn, Lim }

#[derive(ValueEnum, Clone)]
enum CompletionShell { Bash, Zsh, Fish, PowerShell }

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
            Ok(())
        }
        _ => {
            cli.command.execute(cli.force)?;
            println!("✅ Done!");
            Ok(())
        }
    }
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
                    .args(["-i", input, "-vn", "-acodec", "libmp3lame", "-q:a", "2", &output])
                    .run()
            }
            Commands::Convert { input, output } =>
                Ffmpeg::new(force)
                    .args(["-i", input, output])
                    .run(),
            Commands::Split { input, video_output, audio_output } => {
                let v_out = video_output.clone().unwrap_or_else(|| postfix_with_same_ext(input, "_split"));
                let a_out = audio_output.clone().unwrap_or_else(|| postfix_with_ext(input, "_split", "mp3"));
                Ffmpeg::new(force)
                    .args(["-i", input, "-c:v", "copy", "-an", &v_out])
                    .args(["-c:a", "libmp3lame", "-q:a", "2", "-vn", &a_out])
                    .run()
            }
            Commands::Merge { video, audio, output } => {
                let output = output.clone().unwrap_or_else(|| postfix_with_same_ext(video, "_merged"));
                let out_ext = Path::new(&output).extension().unwrap_or_default().to_string_lossy().to_lowercase();
                let audio_codec = probe_audio_codec(audio).unwrap_or_default();
                let incompatible = matches!(
                    (out_ext.as_str(), audio_codec.as_str()),
                    ("mp4" | "mov" | "m4v", "vorbis" | "flac" | "opus" | "alac") |
                    ("webm",                "aac" | "mp3" | "ac3" | "eac3")
                );
                if incompatible {
                    println!("⚠️  Audio codec '{}' is incompatible with .{}, re-encoding to AAC.", audio_codec, out_ext);
                    Ffmpeg::new(force)
                        .args(["-i", video, "-i", audio, "-c:v", "copy", "-c:a", "aac", "-map", "0:v:0", "-map", "1:a:0", &output])
                        .run()
                } else {
                    Ffmpeg::new(force)
                        .args(["-i", video, "-i", audio, "-c", "copy", "-map", "0:v:0", "-map", "1:a:0", &output])
                        .run()
                }
            }
            Commands::Crop { input, output, start, end, copy } => {
                if let Some(s) = start { validate_time(s)?; }
                if let Some(e) = end   { validate_time(e)?; }
                let output = output.clone().unwrap_or_else(|| postfix_with_same_ext(input, "_cropped"));
                let start = start.as_deref().unwrap_or("0");

                let f = Ffmpeg::new(force);
                if *copy {
                    let mut args = vec!["-ss", start, "-i", input, "-c", "copy", "-avoid_negative_ts", "1"];
                    if let Some(e) = end { args.extend(["-to", e]); }
                    args.push(&output);
                    f.args(args).run()
                } else if is_audio_only(input) {
                    let mut args = vec!["-i", input, "-ss", start];
                    if let Some(e) = end { args.extend(["-to", e]); }
                    let (aenc, a_quality): (&str, &[&str]) = audio_encoder_args(input);
                    args.extend(["-c:a", aenc]);
                    args.extend(a_quality.iter().copied());
                    args.push(&output);
                    f.args(args).run()
                } else {
                    let mut args = vec!["-i", input, "-ss", start];
                    if let Some(e) = end { args.extend(["-to", e]); }
                    let (venc, v_quality): (&str, &[&str]) = match probe_video_codec(input).as_deref() {
                        Some("hevc") => ("libx265",    &["-preset", "slow", "-crf", "28"]),
                        Some("vp9")  => ("libvpx-vp9", &["-crf", "33", "-b:v", "0"]),
                        Some("av1")  => { println!("⚠️  AV1 encoding is very slow. Consider using --copy or re-encoding to a faster codec."); ("libaom-av1", &["-crf", "35", "-b:v", "0"]) },
                        _            => ("libx264",    &["-preset", "slow", "-crf", "23"]),
                    };
                    let (aenc, a_quality): (&str, &[&str]) = audio_encoder_args(input);
                    args.extend(["-c:v", venc]);
                    args.extend(v_quality.iter().copied());
                    args.extend(["-c:a", aenc]);
                    args.extend(a_quality.iter().copied());
                    args.push(&output);
                    f.args(args).run()
                }
            }
            Commands::Loud { mode, input, output, percent, integrated } => {
                let output = output.clone().unwrap_or_else(|| postfix_with_same_ext(input, "_loud"));
                match mode {
                    LoudMode::Dyn =>
                        Ffmpeg::new(force)
                            .args(["-i", input, "-af", &format!("loudnorm=I={}:TP=-1:LRA=11", integrated), &output])
                            .run(),
                    LoudMode::Lim => {
                        let stats = Ffmpeg::new(false).args(["-i", input, "-af", "volumedetect", "-f", "null", "-"]).capture()?;

                        let mut hist: Vec<(u32, u64)> = stats.lines()
                            .filter(|l| l.contains("histogram_"))
                            .filter_map(|l| {
                                let p: Vec<&str> = l.split_whitespace().collect();
                                let db = p.get(3)?.strip_prefix("histogram_")?.strip_suffix("db:")?.parse().ok()?;
                                let count = p.get(4)?.parse().ok()?;
                                Some((db, count))
                            }).collect();

                        let boost = if *percent == 0.0 {
                            println!("--- Analyzing Audio (No limiting, boosting to 0 dBFS) ---");
                            let max_volume = stats.lines()
                                .find(|l| l.contains("max_volume:"))
                                .and_then(|l| l.split_whitespace().nth(4))
                                .and_then(|v| v.parse::<f64>().ok())
                                .unwrap_or(0.0);
                            (-max_volume) as u32
                        } else {
                            println!("--- Analyzing Audio (Targeting top {}% samples) ---", percent);
                            hist.sort_by_key(|h| Reverse(h.0));
                            let total: u64 = hist.iter().map(|h| h.1).sum();
                            let target_count = (total as f64 * (percent / 100.0)) as u64;
                            let mut current = 0u64;
                            hist.iter().find_map(|(db, count)| {
                                current += count;
                                if current >= target_count { Some(*db) } else { None }
                            }).unwrap_or(0)
                        };

                        println!("Calculated Boost: {} dB", boost);
                        let filter = if *percent == 0.0 {
                            format!("volume={}dB", boost)
                        } else {
                            format!("volume={}dB,alimiter=limit=0.98:attack=5:release=50", boost)
                        };
                        Ffmpeg::new(force)
                            .args(["-i", input, "-af", &filter, &output])
                            .run()
                    }
                }
            }
            Commands::Completions { .. } => unreachable!(),
        }
    }
}

// -----------------------------------------------------------------------------
// FFMPEG BUILDER
// -----------------------------------------------------------------------------

struct Ffmpeg { args: Vec<String> }

impl Ffmpeg {
    fn new(force: bool) -> Self {
        let mut args = Vec::new();
        if force { args.push("-y".into()); }
        Self { args }
    }

    fn args<I, S>(mut self, args: I) -> Self where I: IntoIterator<Item = S>, S: Into<String> {
        self.args.extend(args.into_iter().map(Into::into));
        self
    }

    fn run(self) -> Result<()> {
        let status = Command::new("ffmpeg").args(&self.args).status().context("FFmpeg failed")?;
        if status.success() { Ok(()) } else { bail!("FFmpeg error.") }
    }

    fn capture(self) -> Result<String> {
        let out = Command::new("ffmpeg").args(&self.args).stderr(Stdio::piped()).output()?;
        Ok(String::from_utf8_lossy(&out.stderr).to_string())
    }
}

// -----------------------------------------------------------------------------
// HELPERS
// -----------------------------------------------------------------------------

fn ensure_ffmpeg_installed() -> Result<()> {
    Command::new("ffmpeg").arg("-version").output().map(|_| ()).map_err(|_| anyhow::anyhow!("ffmpeg not found"))
}

fn validate_time(t: &str) -> Result<()> {
    if !t.contains(':') { bail!("Expected HH:MM:SS"); } Ok(())
}

fn replace_ext(input: &str, new_ext: &str) -> String {
    Path::new(input).with_extension(new_ext).to_string_lossy().into_owned()
}

fn postfix_with_same_ext(input: &str, postfix: &str) -> String {
    let p = Path::new(input);
    let stem = p.file_stem().unwrap().to_string_lossy();
    let ext = p.extension().unwrap_or_default().to_string_lossy();
    build_filename(p, &format!("{stem}{postfix}"), &ext)
}

fn postfix_with_ext(input: &str, postfix: &str, ext: &str) -> String {
    let p = Path::new(input);
    let stem = p.file_stem().unwrap().to_string_lossy();
    build_filename(p, &format!("{stem}{postfix}"), ext)
}

fn build_filename(base: &Path, stem: &str, ext: &str) -> String {
    let mut new = PathBuf::from(base.parent().unwrap_or(Path::new("")));
    new.push(format!("{stem}.{ext}"));
    new.to_string_lossy().into_owned()
}

fn is_audio_only(input: &str) -> bool {
    let ext = Path::new(input)
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    matches!(ext.as_str(), "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "wma")
}

fn probe_codec(file: &str, stream: &str) -> Option<String> {
    let out = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", stream,
               "-show_entries", "stream=codec_name",
               "-of", "default=noprint_wrappers=1:nokey=1", file])
        .output().ok()?;
    let codec = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if codec.is_empty() { None } else { Some(codec) }
}

fn probe_video_codec(file: &str) -> Option<String> { probe_codec(file, "v:0") }
fn probe_audio_codec(file: &str) -> Option<String> { probe_codec(file, "a:0") }

fn audio_encoder_args(file: &str) -> (&'static str, &'static [&'static str]) {
    match probe_audio_codec(file).as_deref() {
        Some("mp3")    => ("libmp3lame", &["-q:a", "2"]),
        Some("opus")   => ("libopus",    &["-b:a", "128k"]),
        Some("vorbis") => ("libvorbis",  &["-q:a", "5"]),
        Some("flac")   => ("flac",       &[]),
        Some("ac3")    => ("ac3",        &[]),
        Some("eac3")   => ("eac3",       &[]),
        _              => ("aac",        &[]),
    }
}

fn map_shell(shell: &CompletionShell) -> Shell {
    match shell {
        CompletionShell::Bash => Shell::Bash,
        CompletionShell::Zsh => Shell::Zsh,
        CompletionShell::Fish => Shell::Fish,
        CompletionShell::PowerShell => Shell::PowerShell,
    }
}