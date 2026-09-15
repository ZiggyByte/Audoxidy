# Memory & CPU Notes

Knowledge base for Audoxidy's playback memory/CPU behavior. **Read this before touching
the audio memory-management code.**

## Status: STABLE — do not modify without re-measuring

The playback memory behavior described here was fixed and verified. The related code
(`MmapSource` in `src/audio/decoder.rs`) is tuned and **must not be changed** without
re-measuring RSS and CPU (see "Verification" below). It is easy to reintroduce either the
unbounded RSS growth or a large CPU regression.

---

## Incident: RSS grew for the whole duration of a song

**Root cause.** Audio files larger than 10 MB are opened via `memmap2::Mmap`
(`MEMMAP_THRESHOLD` in `src/audio/decoder.rs`). Sequential reads faulted every touched
page into the process RSS, and the whole mapping stayed resident until the track's
`FormatReader` was dropped (on track change). RSS therefore grew roughly 1:1 with the
bytes decoded — proportional to file size and duration — instead of plateauing. Changing
tracks released the mapping, which is why RSS dropped on every song change; repeated
manual skips never returned to baseline because each mapping stayed resident until drop.

**Fix.** In `MmapSource::read`, consumed pages are released with `MADV_DONTNEED`
(`unchecked_advise_range`), keeping RSS bounded to the active read window. The advise is
**batched** every `MMAP_ADVISE_CHUNK` (8 MiB) so it costs a handful of syscalls per song
instead of one per read. A backward `seek` reopens the advised block. A regression test
(`mmap_source_releases_pages_and_stays_readable`) asserts a full read and a re-read after
seek are byte-identical.

**Verification.** Pre-fix, a 10 MB WAV via mmap grew RSS 14 → 23 MB; the same file via the
`File` path (<10 MB) showed no growth. Post-fix, the mmap decode stays flat and a repeated
open + full-read + drop cycle does not accumulate. `cargo test` stays green.

**CPU tradeoff.** Releasing page-by-page was costly (`madvise` per read + re-faults). With
the 8 MiB batching the CPU cost is negligible; the earlier +35–50% CPU measurement was a
**debug build** artifact (debug does not optimize DSP/SIMD/resampling). Compare CPU only on
a release build.

**Audio quality.** Unaffected. `MADV_DONTNEED` on a read-only mapping only affects page
residency; the data read is unchanged (asserted by the regression test).

---

## Operational Q&A (expected RSS, channels, format, chunk tuning)

Moved to the planning knowledge base: `.planning/debug/resolved/ram-grows-during-playback.md`
(section "Operational Q&A — expected RSS / channels / format / chunk"). Summary: the
increase plateaus (the ±15% is buffer churn); it scales primarily with sample rate and
channel count and, when crossfade/preload is enabled, with the preload buffer (up to the
~64 MB cap at 384 kHz). File size only adds the ≤8 MB mmap window; the audio format does
not affect RAM. Keep `MMAP_ADVISE_CHUNK` at 8 MiB (lower to trim RSS, never raise).

