//! Shared fixtures for the crate's tests.
//!
//! Everything here keeps a test inside a private directory that this module
//! creates: no fixture writes into the real cover cache, opens the application
//! database, or lands on a memory-backed path. The directory is removed when its
//! guard is dropped, so a failing test cleans up exactly like a passing one.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::db::database::SongData;

/// A private directory for one test, removed when the guard is dropped.
pub(crate) struct TempTestDir {
    path: PathBuf,
}

impl TempTestDir {
    /// Creates a fresh directory under the disk-backed fixture root.
    ///
    /// The label only names the directory so a leftover path reads well: it is
    /// reduced to ASCII alphanumerics and hyphens before it is joined under the
    /// root, so no caller can steer the path out of it.
    pub(crate) fn new(label: &str) -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let root = fixture_root();
        let safe_label: String = label
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '-' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        std::fs::create_dir_all(&root).expect("the fixture root must be creatable");
        loop {
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let candidate = root.join(format!(
                "audoxidy-tests-{}-{sequence}-{safe_label}",
                std::process::id()
            ));
            // A recycled process id can collide with a directory a previous run
            // left behind: take the next sequence instead of reusing it.
            match std::fs::create_dir(&candidate) {
                Ok(()) => return Self { path: candidate },
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("could not create the test directory: {error}"),
            }
        }
    }

    /// Path of the directory this guard holds.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempTestDir {
    fn drop(&mut self) {
        // Never fail here: a panic while already unwinding aborts the process.
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Root every fixture directory is created under.
///
/// `TMPDIR` wins because the documented cargo command points it at a disk-backed
/// location; without it the user cache directory keeps a plain `cargo test` off a
/// memory-backed filesystem, and the platform default is the last resort when no
/// home directory resolves.
fn fixture_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("TMPDIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(dir);
    }
    home_cache_root().unwrap_or_else(|| std::env::temp_dir())
}

/// `$HOME/.cache/audoxidy-test-tmp`, the disk-backed fallback root.
fn home_cache_root() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
    if home.is_empty() {
        return None;
    }
    Some(PathBuf::from(home).join(".cache").join("audoxidy-test-tmp"))
}

/// Builds a song row with the three fields a search or scan fixture identifies.
pub(crate) fn make_song(path: &str, title: &str, artist: &str) -> SongData {
    SongData {
        full_file_path: Arc::from(path.to_string().into_boxed_str()),
        title: Some(Arc::from(title.to_string().into_boxed_str())),
        artist: Some(Arc::from(artist.to_string().into_boxed_str())),
        ..Default::default()
    }
}

/// Writes a 16-bit PCM WAV holding a 440 Hz sine, so a decoder has real audio.
///
/// Every channel carries the same sample, and the amplitude is halved against
/// full scale so a peak stays below the clipping threshold. The header and the
/// sample arithmetic are the ones the decoder and scanner fixtures already
/// build, so both read the same bytes.
pub(crate) fn write_pcm_wav(path: &Path, rate: u32, channels: u16, seconds: f64) {
    let frames = (rate as f64 * seconds) as u32;
    let data_len = frames * channels as u32 * 2;
    let mut wav: Vec<u8> = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&rate.to_le_bytes());
    wav.extend_from_slice(&(rate * channels as u32 * 2).to_le_bytes());
    wav.extend_from_slice(&(channels * 2).to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..frames {
        let sample = ((2.0 * std::f64::consts::PI * 440.0 * i as f64 / rate as f64).sin()
            * 0.5
            * i16::MAX as f64) as i16;
        for _ in 0..channels {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
    }
    std::fs::write(path, wav).expect("the PCM fixture must be writable");
}
