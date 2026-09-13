# Changelog

All notable changes to Audoxidy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [0.8.0] - 2026-09-13

Crate Modernization — mega-release que moderniza cada crate activa a su última versión estable en lotes pequeños y testeados, preservando el pipeline de audio f64 (el único narrowing a f32 sigue en el push al ringbuf). Suite: 311 tests verdes.

### Added

- **ReplayGain** — per-track and per-album loudness normalization with per-source offsets and real-time fallback analysis for untagged files
- **Crossfade / cross-mixing** — automatic (on track change) and manual forward crossfade with equal-power curves and advance pre-loading of the next track
- **Volume fades** — fade-in at the start and fade-out at the end of each song, plus debounce + ramp volume smoothing
- **Silence removal** — trims leading and trailing silence
- StandardCheckbox and NumberStepper custom widgets; adaptive pre-loading cap with GC safety around crossfade
- **Desktop media controls** — MPRIS/SMTC Next/Previous, seek and volume, with album and cover art on the system panel; foundation for future global shortcuts
- **EQ preset management** — save/load/delete presets with JSON import/export
- Volume & Mixing tab in Audio Center — volume smoothing, silence removal, ReplayGain with fixed gain, real-time loudness analysis
- Observability module owning logging init plus a repaired metrics layer that mutates the four atomic counters from structured `target: "audoxidy::metrics"` events
- Lock-free CPAL underrun/latency atomics (buffer size, underruns, ringbuf peak latency) surfaced through `AudioManager`
- Bounded(64) GUI→decoder command channel with a never-drop control policy and a latest-wins atomic Seek slot
- Opt-in `deadlock-detection` cargo feature (`parking_lot::deadlock` watchdog), excluded from default/release builds
- Non-destructive RON config loading (`load_config_from`/`save_config_to`, timestamped backup keep-3, `#[serde(default)]`) with a tracked legacy fixture and regression tests
- Typed OS→app external-control channel (non-panicking sender, hash-stable iced stream adapter); full MPRIS/SMTC Next/Previous, seek and volume; `xesam:album` and `mpris:artUrl file://`
- Serde-defaulted `shortcuts` config section (`ShortcutAction`/`ShortcutBinding`)
- Adaptive F32-first CPAL sample-format selection, named `UnsupportedSampleFormat` rejection, and stable-id device persistence with a non-modal fallback notice
- Planar f64 decode pools (`copy_to_vecs_planar::<f64>`) and a deterministic, device-free test suite: golden 16/24-bit decode, duration fallbacks, planar channel order, 384 kHz/8ch smoke, on-disk `library.db` round trip, resampler frequency response
- Tracked benchmark baseline (`benches/BASELINES.md`, `post-audio-core`) and decision records (`COVERCACHE_LRU_DECISION.md`, `RESAMPLER_FFT_DECISION.md`, `PIPEWIRE_HOST_DECISION.md`, `F64X8_AVX512_DECISION.md`)
- Level-gated per-batch `tracing` spans (decode → resample → DSP → ringbuf push, plus crossfade preload/tail)
- Typed `ConfigError` / `MediaControlsError` with `source()` and annotated database-lock drops

### Changed

- Toolchain pinned (`rust-toolchain.toml`, rustfmt/clippy), package identity completed (MSRV 1.95, GPL-3.0-only, metadata), profiles centralized, `[lints]` enabled
- `tracing`/`tracing-subscriber` adopted project-wide; production `println!`/`eprintln!` and silent `let _` replaced with structured events
- `wide` 1.7 (`blend` → `select`) and `criterion` 0.8.2; `AudioError` moved to `thiserror`
- `rusqlite` 0.40.2 (bundled) with a bounded 32-entry statement cache flushed after scans; robust FTS5 operator escaping
- `sysinfo` 0.39.6 / `souvlaki` 0.8.3 / `lru` 0.18.4 / `url` 2.5.8; RAM path on `MemoryRefreshKind`
- Audio core migrated atomically: `symphonia` 0.6.1, `rubato` 5.0.0, `audioadapter-buffers` 5.2, `cpal` 0.18.2, `ringbuf` 0.5.1 — `Ok(None)`-only EOF, `Track`/`TimeBase` duration with frames/rate fallback, `Channels` enum, explicit `Some(f_cutoff)`, stable-id device resolution
- `CoverCache` now uses the `lru` crate behind its unchanged public API (evaluated and adopted on measured parity + performance)
- Installable as a `src/lib.rs` library plus the binary; benches link the widened engine items
- Migrated PipeWire/PulseAudio integration to dedicated `system_audio.rs`; PipeWire/PulseAudio clock synchronization
- Upgraded Lofty from 0.23.3 to 0.25.1 (yanked version fix)
- Fade defaults adjusted to 2000 ms fade-out / 1000 ms fade-in

### Fixed

- Latent undefined behavior from the unconditional `unsafe set_cpu_extensions(Avx2)` — replaced with a runtime-detected, guarded selector
- Silent audio-center settings and persistence failures now log with context; GUI database-lock drops classified
- Crossfade DRAIN→DECODE window guarded against reintroduced forced deallocations
- Over-claimed formats corrected (APE/Opus/WavPack unsupported by symphonia 0.6.1 → named error)
- Indeterminate duration now yields a non-interactive position rail and suppresses fine seek
- Flaky `utils::memory_manager` tests (bounded `try_lock` retry)
- Crossfade volume fade defaults and fade-out/fade-in timing
- Multi-channel crash and buzz on 7.1 sources
- Sample rate switching without pause/play interrupt
- Pre-load buffer loss on drain transition
- Fade oscillation root cause (double-multiply bug)
- Ring buffer recreation on sample rate change

### Removed

- `hrtf` crate and the dead `pipewire`/`dev` cargo features
- `bumpalo` arena allocator and its direct dependency
- The interleaved `AudioBuffer`/`mix_channels_direct` mixing path (replaced by planar pools)
- Residual dead code and `#[allow(dead_code)]` touched by the migration

### Notes

- The internal pipeline stays f64 end-to-end; the single `as f32` narrowing remains at the ringbuf push (test-asserted).
- The resampler FFT candidate was measured (12–15× CPU) but kept out due to an in-band anti-aliasing regression; sinc stays.
- Native `cpal` PipeWire host and `wide::f64x8`/AVX-512 were evaluated and intentionally not adopted (documented decisions).

## [Unreleased]

_No unreleased changes._

## [0.7.0] - 2026-04-24

### Added

- Playlist module with advanced features — create, edit, reorder, shuffle, persist, drag-and-drop
- Library filter sidebar — browse by folder, artist, album, genre, year with tree navigation
- Context menus across playlist and library
- Multi-selection in library views
- Adaptive artist/album grouping
- Playlist search with keyboard navigation and auto-scroll
- Playback indicators in library views

### Changed

- Rebuilt library filter module from scratch for responsiveness
- Redesigned context menu styling across all views
- Optimized database queries and tag extraction
- Improved playlist navigation and shuffle mode

### Fixed

- Library grid search functionality
- Filter sidebar synchronization with library views
- Keyboard scrolling in library

## [0.6.0] - 2026-03-28

### Added

- Advanced cover art caching system — AVIF compression, LRU cache, background processing
- Thumbnail list view mode for library
- Detailed list view mode with auto-scroll
- Simple list view mode with smart motion and keyboard shortcuts

### Changed

- Refactored codebase — extracted reusable widgets and modules
- Updated Lofty to 0.23.3
- Optimized massive library loading and cache creation

### Fixed

- Grid and simple list view bugs
- Library navigation edge cases

## [0.5.1] - 2026-03-17

### Added

- Grid view mode for library with exact FLAC metadata
- Complex sorting capabilities
- Keyboard shortcuts for album list display and song/album deletion
- Real-time library statistics updates

### Fixed

- Library keyboard scrolling

## [0.5.0] - 2026-03-12

### Added

- Library Grid view mode with metadata support
- Keyboard navigation across library views

### Changed

- Improved library layout and navigation

## [0.4.2] - 2026-03-12

### Added

- Audio library with bit-depth support and improved metadata formats

### Changed

- Performance and layout improvements in library grid mode
- Improved player response time

### Fixed

- Audio control button adjustments

## [0.4.1] - 2026-03-08

### Added

- Completed migration of Player module from egui to Iced 0.14
- Multi-channel volume adjustment for 5.1+ sources on stereo output

### Changed

- Migrated GUI from egui to Iced for improved performance
- Audio and design control adjustments

## [0.3.0] - 2026-02-20

### Added

- Multi-channel DSP engine with 15 high-performance audio effects
- Advanced EQ preset loading/saving system
- 21-band and 31-band IEC standard equalizers
- Compressor, limiter, reverb, stereo expander, noise gate
- Sub-bass, mid-bass, and voice boost enhancement

### Changed

- Improved audio engine stability

## [0.2.0] - 2026-02-19

### Added

- Initial public release
- Core audio engine with CPAL output
- Iced GUI with dark theme
- 21-band and 31-band IEC standard equalizers
- Basic format support (MP3, FLAC, WAV, OGG)
- SQLite music library with search
- ReplayGain volume normalization
