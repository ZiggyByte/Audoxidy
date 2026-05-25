# Audoxidy

Audoxidy is a modern, high-fidelity audio player for Linux, Windows, and macOS, written in Rust. It aims to provide bit-perfect playback, advanced DSP effects, and a sleek, customizable user interface built with **Iced 0.14**.

## Features

- **High-Fidelity Audio Engine**: Built on `cpal` and `rubato` for low-latency, bit-perfect playback.
- **Advanced DSP**: Includes equalizer, compressor, reverb, multi-band stereo expander, noise gate, limiter, and more.
- **Smart Channel Mixing**: Automatically handles multi-channel audio (2.1, 5.1, 7.1) and downmixing.
- **Modern UI**: Powered by `iced` 0.14 + `iced_aw`, with dynamic theming and responsive design.
- **Format Support**: Supports MP3, FLAC, WAV, OGG, Opus, AAC, APE, WavPack and more via `symphonia` + `lofty`.
- **ReplayGain**: Automatic volume normalization with track/album gain from metadata tags.
- **AVIF Cover Art**: Carátulas comprimidas en AVIF con caché LRU y anti-OOM.
- **SQLite Library**: Biblioteca musical relacional con FTS5, playlists y persistencia de sesiones.

## Quick Start

```bash
# Prerequisites (Linux)
sudo apt install libasound2-dev libpipewire-0.3-dev  # ALSA + PipeWire

# Build and run
cargo build --release
cargo run --release
```

The config file is auto-created at `~/.config/audoxidy/config.ron` on first run.

## Architecture

- **`src/audio/`** — Motor de audio: engine (CPAL + ringbuf), decodificador Symphonia, cadena DSP, gestor de dispositivos, presets EQ.
- **`src/db/`** — Base de datos SQLite con esquema relacional, FTS5, scanner de biblioteca con `lofty`.
- **`src/gui/`** — Interfaz de usuario Iced 0.14: reproductor, playlist, biblioteca, centro de audio, widgets.
- **`src/utils/`** — Utilidades: interner de cadenas (Arc dedup), gestor de memoria RAM, carátulas AVIF, configuración RON.
- **`src/integrations/`** — Controles multimedia del sistema via `souvlaki` (MPRIS en Linux, SMTC en Windows).

## Build Requirements

- **Rust**: Edición 2024 (stable)
- **Linux**: `libasound2-dev` (ALSA), opcional `libpipewire-0.3-dev` (PipeWire)
- **Windows/macOS**: No se requieren dependencias adicionales

## Key Crates

| Crate | Propósito |
|---|---|
| `cpal` | Audio output de baja latencia multiplataforma |
| `symphonia` | Decodificación de audio (MP3, FLAC, WAV, OGG, AAC, etc.) |
| `rubato` | Resampling asincrónico de alta calidad (SincInterpolation) |
| `iced` | GUI multiplataforma con rendering basado en wgpu |
| `rusqlite` | SQLite con FTS5 para búsqueda de texto completo |
| `lofty` | Lectura de metadatos y tags (ID3v2, Vorbis, APE, etc.) |

## Installation

### Build

```bash
cargo build --release
```

### Run

```bash
cargo run --release
```

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for details on our code of conduct, and the process for submitting pull requests to us.

## License

This project is licensed under the **GNU General Public License v3.0** - see the [LICENSE](LICENSE) file for details.

## Author

**ZiggyCrue** - *Initial Work & Main Developer*
