# ff — Practical FFmpeg Wrapper

**ff** is a small, opinionated CLI that makes everyday `ffmpeg` tasks faster, safer, and easier to remember.

It wraps common media operations into clean subcommands so you don’t have to look up long `ffmpeg` invocations every time.

---

## 🧠  Why This Exists

Because nobody remembers commands like:

```
ffmpeg -i input.mp4 -map 0:v:0 -map 1:a:0 -c:v copy -c:a aac output.mp4
```

...but we run them constantly.

  This is **not** a replacement for FFmpeg. It is a memory aid. When you need full control, just use `ffmpeg`.

---

## ✨ Features

* 🎧 Extract audio from video with sane defaults
* 🔄 Convert between formats (auto-detected by extension)
* 🎬 Split media into separate video/audio tracks
* 🧩 Merge external audio + video safely (with codec compatibility handling)
* ✂️ Precise or fast trimming (re-encode or keyframe copy)
* 🔊 Smart loudness normalization (speech **or** music workflows)
* ⚡ Automatically chooses reasonable encoders when re-encoding is required
* 🐚 Shell completions for Bash / Zsh / Fish / PowerShell
* 💬 Keeps native `ffmpeg` output (no hidden magic)

---

## 📦 Requirements

* `ffmpeg` must be installed and available in your `PATH`
* `ffprobe` (usually bundled with ffmpeg)

Check:

```bash
ffmpeg -version
```

---

## 🔧 Installation

### Option 1 — Download Binary (Recommended)

Download the latest binary for your platform from the [Releases](https://github.com/TeWu/ff/releases/latest) page:

| Platform | File |
|----------|------|
| Linux (x86_64) | `ff-x86_64-unknown-linux-gnu` |
| macOS (Intel) | `ff-x86_64-apple-darwin` |
| macOS (Apple Silicon) | `ff-aarch64-apple-darwin` |
| Windows | `ff-x86_64-pc-windows-msvc.exe` |

**Windows:**

Rename to `ff.exe` and place it somewhere on your `PATH`.

**Linux / macOS:**

```bash
chmod +x ff-x86_64-unknown-linux-gnu
mv ff-x86_64-unknown-linux-gnu ~/.local/bin/ff
```

### Option 2 — Build from Source

Requires [Rust](https://rustup.rs) to be installed.

```bash
git clone https://github.com/TeWu/ff.git
cd ff
cargo build --release
mv target/release/ff ~/.local/bin/
```

---

## 🎵 Extract Audio

```bash
ff extract <INPUT> [OUTPUT]
```

Extracts the audio track as an MP3 file. Output defaults to `<INPUT_BASENAME>.mp3`.

```bash
ff extract video.mp4
ff extract video.mp4 audio.mp3
```

---

## 🔄 Convert between formats

```bash
ff convert <INPUT> <OUTPUT>
```

Converts a media file to a different format. The output format is determined by the file extension.

```bash
ff convert input.flac output.mp3
ff convert input.mov output.mp4
ff convert input.mp4 output.webm
```

---

## ✂️ Split Video and Audio into separate files

```bash
ff split <INPUT> [VIDEO_OUTPUT] [AUDIO_OUTPUT]
```

Produces a video-only file and an audio-only file from a single input. Defaults: `<INPUT_BASENAME>_split.<ext>` for video and `<INPUT_BASENAME>_split.mp3` for audio.

```bash
ff split movie.mp4
ff split movie.mp4 video.mp4 audio.mp3
```

Video is stream copied losslessly. Audio is always re-encoded to MP3 — a deliberate choice for maximum compatibility.

---

## 🔗 Merge Audio + Video

```bash
ff merge <VIDEO> <AUDIO> [OUTPUT]
```

Combines a video stream and an audio stream into a single file. Audio is copied as-is when codec is supported by the chosen output format; otherwise it is re-encoded to AAC automatically. Output defaults to `<VIDEO_BASENAME>_merged.<video ext>`.

```bash
ff merge video.mp4 audio.m4a
ff merge v.mp4 a.flac final.mp4
```

---

## 🪚 Crop / Trim Media

```bash
ff crop [-s <START>] [-e <END>] [--copy] <INPUT> [OUTPUT]
```

Cuts a segment from a media file. By default performs precise trimming via re-encoding. Use `--copy` for fast keyframe-aligned trimming without re-encoding. Output defaults to `<INPUT_BASENAME>_cropped.<ext>`.

| Flag | Description |
|------|-------------|
| `-s`, `--start` | ⏱️ Start timestamp in `HH:MM:SS` format (default: beginning) |
| `-e`, `--end` | ⏱️ End timestamp in `HH:MM:SS` format (default: end) |
| `--copy` | Fast mode — no re-encode, less precise (cuts on keyframes only) |

```bash
ff crop -e 00:02:00 input.mp4
ff crop -s 00:00:10 -e 00:00:20 input.mp4 out.mp4
ff crop -s 00:01:00 -e 00:02:00 --copy input.mp4
```

---

## 🔊 Loudness Processing

Two different modes:

### `dyn` — Dynamic Normalization (Speech / Podcasts)

Applies loudness normalization (EBU R128 via `loudnorm`). Quieter parts get louder and peaks are tamed. Does **not** preserve original sound dynamics. Best for speech and podcasts.

```bash
ff loud dyn [-I] <INPUT> [OUTPUT]
```

| Flag | Default | Description |
|------|---------|-------------|
| `-I`, `--integrated` | `-14` | Target integrated loudness in LUFS (range: −70 to −5) |

```bash
ff loud dyn speech.mp3
ff loud dyn -I -9 speech.mp3
ff loud dyn -I -11 speech.mp3 out.mp3
```

### `lim` — Percentile-Based Limiting (Music)

Applies a steady volume boost so that a given, small percentage of samples hit a limiter. Preserves dynamics. Best for music.

```bash
ff loud lim [--top] <INPUT> [OUTPUT]
```

| Flag | Default | Description |
|------|---------|-------------|
| `--top` | `0` | Percentage of loudest samples to push against the limiter |

```bash
ff loud lim music.mp3
ff loud lim --top 5 music.mp3
ff loud lim --top 20 music.mp3 out.mp3
```

When `--top 0` (default), boosts to 0 dBFS with no limiting.

---

## 🐚 Shell Completions

Generate completions:

```bash
ff completions bash > ~/.ff-complete.sh
echo 'source ~/.ff-complete.sh' >> ~/.bashrc
```

Supported shells:

* bash
* zsh
* fish
* powershell

---

## 📁 Output Naming Defaults

When no output file is specified, `ff` generates one automatically:

| Command | Default output |
|---------|----------------|
| 🎵 `extract` | `<input>.mp3` |
| ✂️ `split` | `<input>_split.<ext>` + `<input>_split.mp3` |
| 🔗 `merge` | `<video>_merged.<ext>` |
| 🪚 `crop` | `<input>_cropped.<ext>` |
| 🔊 `loud` | `<input>_loud.<ext>` |

---

## ⚠️ Notes on Re-Encoding

The tool tries to:

* Copy streams when safe
* Re-encode only when required
* Choose codecs based on the input media

This prevents broken outputs while keeping operations fast when possible.
