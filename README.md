# Audoxidy

Audoxidy is a modern, high-fidelity audio player for Linux, Windows, and macOS, written in Rust. It aims to provide bit-perfect playback, advanced DSP effects, and a sleek, customizable user interface.

## Features

- **High-Fidelity Audio Engine**: Built on `cpal` and `rubato` for low-latency, bit-perfect playback.
- **Advanced DSP**: Includes equalizer, compressor, reverb, and more.
- **Smart Channel Mixing**: Automatically handles multi-channel audio (2.1, 5.1, 7.1) and downmixing.
- **Modern UI**: Powered by `egui`, with dynamic theming and responsive design.
- **Format Support**: Supports MP3, FLAC, WAV, OGG, and more via `symphonia`.

## Installation

### Prerequisites

- Rust (latest stable)
- ALSA development headers (Linux): `libasound2-dev`

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
