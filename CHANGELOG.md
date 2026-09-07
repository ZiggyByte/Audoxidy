# Changelog

All notable changes to Audoxidy will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/), and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Volume & Mixing tab in Audio Center — volume smoothing, silence removal, ReplayGain with fixed gain, real-time loudness analysis
- Crossfade system — manual and automatic forward crossfade with equal-power curves
- Pre-loading engine — next track decoded in advance for gapless transitions
- EQ preset management — save/load/delete presets, import/export via JSON
- StandardCheckbox and NumberStepper custom widgets
- Adaptive pre-loading cap and GC safety for crossfade
- PipeWire/PulseAudio clock synchronization
- 228+ tests across audio engine, DSP, widgets, and crossfade

### Changed

- Migrated PipeWire/PulseAudio integration to dedicated `system_audio.rs`
- Upgraded Lofty from 0.23.3 to 0.25.1 (fix yanked version)
- Volume smoothing uses debounce + ramp instead of linear interpolation
- Fade defaults adjusted to 2000ms fade-out / 1000ms fade-in

### Fixed

- Crossfade volume fade defaults and fade-out/fade-in timing
- Multi-channel crash and buzz on 7.1 sources
- Sample rate switching without pause/play interrupt
- Pre-load buffer loss on drain transition
- Fade oscillation root cause (double-multiply bug)
- Ring buffer recreation on sample rate change

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
