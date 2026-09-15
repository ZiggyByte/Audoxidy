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

## Q&A: expected RSS increase (post-fix, crossfade/preload enabled)

Drivers (see `src/audio/decoder.rs`):

- **Ring buffer:** 8 MiB fixed (2 MiB in low-resource) — constant, independent of channels.
- **`MMAP_ADVISE_CHUNK` = 8 MiB:** only for files >10 MiB; up to ~8 MB extra, independent of
  sample rate or channels.
- **Crossfade preload buffer:** `min(8 s × rate × channels, 8,000,000)` f64 samples
  (`PRELOAD_MAX_SAMPLES` = 8M, ~64 MB cap). The dominant, rate-dependent allocation.
- **Tail buffer:** `0.2 s × rate × channels` f64 samples.
- **Planar pools / resampler / DSP state:** scale with rate × channels (few MB).
- The increase **plateaus**; it no longer grows with playback time. The ±15% fluctuation is
  normal ring-buffer/pool churn.

| Scenario | ~3 ch | ~4 ch | ~5 ch | ~6 ch | ~7 ch | ~8 ch |
|---|---|---|---|---|---|---|
| <10 MB @ 44.1 kHz | ~20 MB | ~23 | ~26 | ~29 | ~32 | ~35 |
| <10 MB @ 384 kHz | ~85 MB | ~88 | ~90 | ~93 | ~96 | ~100 |
| 22 MB @ 44.1 kHz | ~28 MB | ~31 | ~34 | ~37 | ~40 | ~43 |
| 22 MB @ 384 kHz | ~93 MB | ~96 | ~98 | ~101 | ~104 | ~108 |

Notes:
- At 384 kHz and ≥3 channels the preload buffer hits the 8M-sample cap (~64 MB), so that
  buffer stops growing with channels; the tail buffer and pools keep scaling.
- File size only ever adds the ≤8 MB mmap window — nothing else.
- With crossfade/preload disabled the preload buffer is 0 and these totals roughly halve.
- Low-resource mode reduces the ring buffer (2 MiB) and preload.

### Does the audio format affect RAM?

Directly, **no**. FLAC/MP3/WAV/OGG all decode to the same PCM at the output rate, so the
steady-state RSS is the same. It matters only indirectly: file size (the 10 MB mmap
threshold), and channel count/bit depth. The codec affects **CPU**, not RAM.

### `MMAP_ADVISE_CHUNK`: keep 8 MiB?

**Keep 8 MiB; if trimming RSS, lower it (4 MiB), never raise it.** With batching the
`madvise` cost is a handful of syscalls per song, so raising it buys no measurable CPU and
only raises the resident file pages. Lowering it trades negligible CPU for slightly lower
RSS. This knob is second-order — the real levers are the crossfade preload buffer, the ring
buffer, sample rate, and channel count.
