# Memory analysis — 280 MB peak during playback at 384 kHz

Last updated: 2026-11-15

Scope: Audoxidy playback and browsing at 384,000 Hz stereo, with a library of 27,743 songs
and a full-library playlist. Measured peak: ~280 MB. Floor after reclaim: ~210 MB. Earlier
measured ceiling: ~190 MB.

This document records the evidence and the budget. It is a static, code-based attribution;
the numbers come from the live application.

---

## 1. Conclusion

**There is no leak.** The peak is accumulated retention that the GC returns when it runs, and
the 210 MB floor is the capacity the engine intentionally keeps. Every large allocation has a
fixed ceiling; none grows with playback time or with the number of track changes.

The cover cache adds **one** large holder: the decoded-cover store, bounded at 32 MiB (reduced
from 48 MiB; the measured 1080p / 220 px budget is 16.26 MB, so the ceiling only binds at high
DPI) and deliberately untouched by the GC. That, plus the playlist and filter-index copies for a
27,743-song library, explains the ~70–90 MB delta over the earlier 190 MB measurement. The audio
engine buffers are unchanged by the metadata and cover-cache work.

---

## 2. Budget at the measured configuration

Assumes stereo. The last column shows the 8-channel case for the buffers whose size scales
with channels.

| Holder | Stereo @ 384 kHz | 8 ch @ 384 kHz | Lifetime |
|---|---:|---:|---|
| Promoted preload head (one buffer; roles exchanged, cap = crossfade max + 2 s, clamped to 8 s) | 18.4 MB | 64 MB (8 M-sample ceiling) | Reused across tracks; no copy, no second allocation |
| Audio ring buffer (recreated as `rate × channels × 2 s`) | 6.1 MB | 24.6 MB | Constant after the rate change |
| Ring buffer if the device rate never triggers a recreate | 32 MB | 32 MB | Constant |
| Mix tail buffer (200 ms) | 1.2 MB | 4.9 MB | Constant |
| DSP chain (reverb FDN, EQ, compressor, limiter) | ~1.8 MB | ~1.8 MB | Constant |
| **Decoded-cover store** | **≤ 32 MB** (~16 MB at 1080p / 220 px) | ≤ 32 MB | Budget-bounded |
| iced cover textures (viewport + 50 % margin) | 25–50 MB | 25–50 MB | Viewport-bounded |
| Player cover handle (400 px cache) | ~0.6 MB | ~0.6 MB | One per track |
| Playlist (`PlaylistSongRef`, shared `Arc`) + filtered view | ~7 MB | ~7 MB | While the playlist is loaded |
| Filter index (arena, 19,881 nodes) | ~1 MB | ~1 MB | Never dropped |
| SQLite page cache (2 conns) + mmap window | 8–24 MB | 8–24 MB | Constant |
| Interner (purges at 10 MB) | ≤ 10 MB | ≤ 10 MB | Bounded |
| Base private heap (fonts, GPU buffers, runtime, MPRIS) | ~30 MB | ~30 MB | Constant |
| Shared (release binary, GPU drivers, fonts) | ~60–90 MB | ~60–90 MB | Constant |

Sum: **~230–290 MB peak**, which matches the measured 280 MB. The 210 MB floor is the same
budget with the cover store and the viewport textures evicted and the interner/playlist caches
cleared.

### Why the preload caps are what they are

`predecode_cap_frames_for` (`src/audio/decoder.rs`) computes

```
cap_ms = clamp(xfade_max_ms + 2000, 0, 8000)
samples = min((cap_ms / 1000) * out_rate * channels, 8_000_000)
```

With the persisted settings (`crossfade_auto_ms = 250`, `crossfade_manual_ms = 1000`; the manual
value counts even while manual crossfade is disabled), `xfade_max_ms = 1000`, so `cap_ms = 3000`
→ 2,304,000 samples at 384 kHz stereo → **18.4 MB** in f64. The 8 M-sample ceiling is 64 MB. The
cap dates from the crossfade redesign (commit `3eacdcf`).

---

### The `[AUDIO_HEALTH]` diagnostic

`[AUDIO_HEALTH] open_preload_track tomó N ms` measured opening and probing the next file on the
decoder thread; it fired above 10 ms. On a slow mount, 18 ms is normal disk latency. The audible
cut correlated with that block: while the decoder opened the next file, it stopped refilling the
ring. The promotion also copied the whole head on that thread. The head now uses one buffer, and
the preload open/probe runs on a helper thread (`fd51cd5`), so the decoder never blocks on it; the
warning was removed because it no longer measures an audio-thread operation. If a cut ever
reappears, look at the preload decode batch or the disk, not the log.

The same 15 s window prefetches the next cover from the 400×400 AVIF cache (a few KB); the
library decodes it down to 220 px for its textures and the player uses the 400 px file. A
short-lived experiment decoded it synchronously through the bounded funnel (`8f103e1`) to cap a
future larger source, but that decode competed with the preload open at the same moment and was
reverted (`34491e4`); `Handle::from_path` decodes the 400 px file off the GUI thread. It was not
the cause of the memory spike.

## 3. Why the peak persists while changing tracks

`Core::evaluate_gc` (`src/core/maintenance.rs`) runs the purge on a 60 s timer and on RAM
pressure. It returns early when either the scanner is running or `gc_blocked_by_preload` is true
(`src/gui/app/update/memory.rs`), which covers the preload's last 15 s plus 20 s after the
transition. Consecutive track changes therefore leave few windows for the timer, and the GC's own
500 MB app cap never fires at 280 MB.

When it does run, `handle_memory` (`src/gui/app/update/memory.rs`) unloads the inactive lists,
clears the virtualization caches, drops the decoded covers outside the visible window when the
library is unfocused, clears the interner and purges the audio preload, then calls
`malloc_trim(0)`. The visible cover window is kept, so returning to the same scroll position
stays instant.

---

## 4. No-leak evidence

- No per-song audio cache exists: no waveform, spectrum, FFT or play-history structure in
  `src/audio`; the decoder state is dropped at end of track.
- `ShuffleSession.history` grows ~8 bytes per played track — negligible, and the only monotonic
  per-play growth found.
- Player metadata fields are overwritten per track, not accumulated.
- The interner self-purges at 10 MB.
- Every large buffer has a ceiling (table above); none is proportional to elapsed playback.

These conclusions match an earlier attribution study recorded with the benchmark notes, which
predicted the same shape before the live measures existed.

---

## 5. Retention levers and their visible impact

Ordered by value/risk. **None of them affects text rendering, cover images once decoded, or
window navigation.** The only visible effect is a possible re-decode or re-query when returning
to a view.

| Lever | Expected saving | Visible impact | Status |
|---|---:|---|---|
| Trim the decoded-cover store when the library is unfocused (keep the current window) | up to the store size (≤32 MB; ~16 MB at 1080p) | Returning to the library keeps the window; scrolling into the trimmed band re-decodes asynchronously | **Applied (`2a6d0fb`)** |
| Drop `filter_index` and `tree_data` on purge when the library is unfocused | 5–15 MB | The filter panel would be empty until a reload; rejected | **Not used; index compacted instead (`5807a7f`)** |
| `shrink_to_fit` on the preload buffers from an idle path | up to 37 MB | None, but freeing the capacity forces the next preload to grow it again | **Not used; one reused buffer instead (`482cee9`)** |
| Reduce the `+2000 ms` preload margin | ~12 MB at 384 kHz stereo | The mix would cover less of the song; requires a listening test | **Retained at +2000 ms** |
| Background-purge allocator (mimalloc/jemalloc) | Allocator-dependent | None | **Not used; new dependency** |

The decoded-cover store cannot be removed without losing the scroll-back guarantee; the budget
lever is its ceiling, not its existence.

### What was actually recovered

The 340 → 250 MB figure is partly the reversal of a regression introduced by an early idle
release: freeing the preload buffers made the next preload reallocate them, producing the 340 MB
spike. The honest comparison is against the **280 MB baseline** measured before those changes:

| Change | Steady saving | Notes |
|---|---:|---|
| One preload head instead of two (`482cee9`) | ~18.4 MB | 64 MB at 8 channels; reused across tracks |
| Filter index arena (`5807a7f`) | ~3 MB | 19,881 nodes measured on the real library |
| Playlist filtered view sharing `Arc` (`48a6d0c`) | ~5.5 MB while a query matches everything | transient |
| Cover trim on GC (`2a6d0fb`) | ~5–10 MB when the library is unfocused | re-decodes on return |
| Cover ceiling 32 MiB (`98a769e`) | 0 at 1×; up to 16 MB at high DPI | validated at 4K |
| **Total steady** | **~21–25 MB** | measured peak 280 → 250 (~30 MB) |

The 250 MB peak is real but modest; the 340 MB figure was the idle-release regression, not a
recovered baseline. The remaining large holders are the 18.4 MB inherent preload head, iced's
viewport textures, SQLite and the interner; the last two are kept by policy.

### Implementation status

- **Cover-store trim — applied (`2a6d0fb`).** With the library out of focus the GC drops the
  decoded covers outside the visible window and keeps the window. Returning to the same scroll
  position is unchanged; scrolling into the trimmed band re-decodes those covers asynchronously
  (1–3 ms per 220 px cover). The earlier source-text guard was replaced by the behavioural test
  that pins window retention.
- **Filter index — compacted, not dropped (`5807a7f`).** The panel must stay visible, so the
  module is never freed.
- **Preload head — one reused buffer (`482cee9`).** The two buffers exchange roles with
  `mem::swap`, so only one head capacity exists and the drained buffer is reused. The promotion
  no longer copies. The manual `Load` path no longer frees the capacity either (`25a0294`).
- **Preload margin — retained at +2000 ms.**
- **Playlist filtered view — shared `Arc<PlaylistSongRef>` (`48a6d0c`).** The filtered view clones
  pointers instead of ~200-byte structs (~5.5 MB → ~220 KB for a full match). Mutations use
  `Arc::make_mut` only on the changed song.

### Filter index — compacted instead of dropped

Measured on the real library: 19,881 nodes across the five trees (genre 2,995; artist 2,630;
album 4,502; year 3,805; folder 5,949). The old tree stored a `String` label and a full-path
`String` id per node, and `apply_view` cloned the active tree while the flattened view copied the
strings again.

The arena (`5807a7f`) stores `Arc<str>` labels shared with the database's interned strings, a
`u32` identity and parent index per node, and `u32` indices in `tree_data` and the flattened
view. The panel is never emptied and never reloaded: `apply_view` only computes a visibility
mask. Estimated cost drops from ~4 MB to ~1 MB; the path is rebuilt only when a node is clicked.

---

## 6. How to confirm live

1. Run the release build manually.
2. Reproduce: 384 kHz output, one playlist of the full library, change tracks for several
   minutes without navigating.
3. Capture `grep -E 'Rss|Pss|Pss_Anon|Private_Dirty' /proc/<pid>/smaps_rollup` at peak, after
   the 60 s GC, and after a manual purge.
4. Attribute the delta against the budget table above: the preload head, the cover store and the
   playlist/filter structures are the only holders large enough to move the number by tens of MB.

---

## 7. References

| Topic | Location |
|---|---|
| Predecode cap | `src/audio/decoder.rs` (`predecode_cap_frames_for`) |
| Preload promotion (head roles exchanged) | `src/audio/decoder.rs` (`promote_preload_head`) |
| Preload open thread | `src/audio/decoder.rs` (`open_preload_track_blocking`) |
| Ring buffer size and recreate | `src/audio/engine.rs` |
| GC guard | `src/gui/app/update/memory.rs` |
| GC policy | `src/core/maintenance.rs` |
| Cover budget | `src/utils/covers.rs` (`COVER_BUDGET_MAX_BYTES`) |
| Cover store ledger | `src/gui/library/cover_cache.rs` |
| Interner cap | `src/utils/interner.rs` |
